use ferriskey::{Error, ErrorKind, FromValue, Value};
use serde::de::DeserializeOwned;
use threatflux_vertex_rust_sdk::{GenerateContentRequest, GenerateContentResponse};

#[derive(Debug, Clone)]
pub struct SearchDocument {
    pub id: String,
    pub request: Option<GenerateContentRequest>,
    pub response: Option<GenerateContentResponse>,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Default)]
pub struct ValkeySearchResult {
    pub total_results: usize,
    pub documents: Vec<SearchDocument>,
}

#[derive(Debug, Clone, Default)]
struct SearchDocumentFields {
    request: Option<GenerateContentRequest>,
    response: Option<GenerateContentResponse>,
    embedding: Option<Vec<f32>>,
}

impl FromValue for ValkeySearchResult {
    fn from_value(v: &Value) -> Result<Self, Error> {
        let Value::Array(items) = v else {
            return Err(Error::from((
                ErrorKind::TypeError,
                "Expected top-level RESP Array from FT.SEARCH response",
            )));
        };

        if items.is_empty() {
            return Err(Error::from((
                ErrorKind::TypeError,
                "FT.SEARCH returned an empty array, missing total count metadata",
            )));
        }

        let total_value = items.first().ok_or_else(|| {
            Error::from((
                ErrorKind::TypeError,
                "FT.SEARCH array is missing the first element",
            ))
        })?;
        let total_value = total_value.as_ref().map_err(|_| {
            Error::from((
                ErrorKind::TypeError,
                "FT.SEARCH total count was not decodable",
            ))
        })?;
        let total_results = usize::from_value(total_value)?;

        let documents = items
            .iter()
            .skip(1)
            .map(|item| {
                let value = item.as_ref().map_err(|_| {
                    Error::from((
                        ErrorKind::TypeError,
                        "Failed to read document item from array",
                    ))
                })?;
                SearchDocument::from_value(value)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            total_results,
            documents,
        })
    }
}

impl FromValue for SearchDocument {
    fn from_value(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Array(items) => {
                let id = items.first().ok_or_else(|| {
                    Error::from((
                        ErrorKind::TypeError,
                        "Encountered an empty search document array",
                    ))
                })?;

                let id = parse_text_value(id.as_ref().map_err(|_| {
                    Error::from((
                        ErrorKind::TypeError,
                        "Encountered an invalid search document identifier",
                    ))
                })?)?;
                let fields = items
                    .get(1)
                    .and_then(|item| item.as_ref().ok())
                    .unwrap_or(&Value::Nil);
                let fields = SearchDocumentFields::from_value(fields)?;

                Ok(Self {
                    id,
                    request: fields.request,
                    response: fields.response,
                    embedding: fields.embedding,
                })
            }
            Value::Map(items) => {
                let (id_value, payload_value) = items.first().ok_or_else(|| {
                    Error::from((
                        ErrorKind::TypeError,
                        "Encountered an empty search document map",
                    ))
                })?;
                let fields = SearchDocumentFields::from_value(payload_value)?;

                Ok(Self {
                    id: parse_text_value(id_value)?,
                    request: fields.request,
                    response: fields.response,
                    embedding: fields.embedding,
                })
            }
            _ => Ok(Self {
                id: parse_text_value(v)?,
                request: None,
                response: None,
                embedding: None,
            }),
        }
    }
}

impl FromValue for SearchDocumentFields {
    fn from_value(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Array(items) => {
                let mut fields = Self::default();
                for chunk in items.chunks(2) {
                    if chunk.len() != 2 {
                        continue;
                    }

                    let field_name = parse_field_name(chunk[0].as_ref().map_err(|_| {
                        Error::from((
                            ErrorKind::TypeError,
                            "Encountered an invalid search field name",
                        ))
                    })?)?;
                    let field_value = chunk[1].as_ref().map_err(|_| {
                        Error::from((
                            ErrorKind::TypeError,
                            "Encountered an invalid search field value",
                        ))
                    })?;

                    match field_name {
                        StoredField::EncodedRequest => {
                            fields.request = Some(parse_json_value(field_value)?)
                        }
                        StoredField::EncodedResponse => {
                            fields.response = Some(parse_json_value(field_value)?)
                        }
                        StoredField::PromptVec => {
                            fields.embedding = Some(parse_embedding_value(field_value)?)
                        }
                    }
                }
                Ok(fields)
            }
            Value::Map(items) => {
                let mut fields = Self::default();
                for (field_name, field_value) in items {
                    match parse_field_name(field_name)? {
                        StoredField::EncodedRequest => {
                            fields.request = Some(parse_json_value(field_value)?)
                        }
                        StoredField::EncodedResponse => {
                            fields.response = Some(parse_json_value(field_value)?)
                        }
                        StoredField::PromptVec => {
                            fields.embedding = Some(parse_embedding_value(field_value)?)
                        }
                    }
                }
                Ok(fields)
            }
            Value::Nil => Ok(Self::default()),
            _ => Ok(Self::default()),
        }
    }
}

enum StoredField {
    EncodedRequest,
    EncodedResponse,
    PromptVec,
}

fn parse_field_name(value: &Value) -> Result<StoredField, Error> {
    match parse_text_value(value)?.as_str() {
        "encoded_request" => Ok(StoredField::EncodedRequest),
        "encoded_response" => Ok(StoredField::EncodedResponse),
        "prompt_vec" => Ok(StoredField::PromptVec),
        _ => Err(Error::from((
            ErrorKind::TypeError,
            "Encountered an unsupported cached field",
        ))),
    }
}

fn parse_json_value<T>(value: &Value) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let raw_text = parse_text_value(value)?;
    serde_json::from_str(&raw_text).map_err(|err| {
        Error::from((
            ErrorKind::TypeError,
            "Failed to decode JSON payload from FT.SEARCH result",
            err.to_string(),
        ))
    })
}

fn parse_text_value(value: &Value) -> Result<String, Error> {
    let raw_text = String::from_value(value).map_err(|_| {
        Error::from((
            ErrorKind::TypeError,
            "Encountered a non-string value while decoding a search result field",
        ))
    })?;

    if let Ok(unquoted) = serde_json::from_str::<String>(&raw_text) {
        return Ok(unquoted);
    }

    Ok(raw_text)
}

fn parse_embedding_value(value: &Value) -> Result<Vec<f32>, Error> {
    let bytes = match value {
        Value::BulkString(bytes) => bytes.as_ref().to_vec(),
        Value::SimpleString(text) => text.as_bytes().to_vec(),
        _ => {
            return Err(Error::from((
                ErrorKind::TypeError,
                "Encountered a non-binary value while decoding an embedding vector",
            )));
        }
    };

    if !bytes.len().is_multiple_of(4) {
        return Err(Error::from((
            ErrorKind::TypeError,
            "Embedding bytes were not aligned to f32",
        )));
    }

    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use threatflux_vertex_rust_sdk::{GenerateContentRequest, GenerateContentResponse};

    #[test]
    fn parses_typed_search_result_documents() {
        let request = GenerateContentRequest::new("test");
        let response = GenerateContentResponse {
            candidates: vec![],
            usage_metadata: None,
            grounding_metadata: None,
        };
        let request_json = serde_json::to_string(&request).unwrap();
        let response_json = serde_json::to_string(&response).unwrap();
        let embedding = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
        let embedding_bytes = embedding
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();

        let payload = Value::Array(vec![
            Ok(Value::Int(1)),
            Ok(Value::Array(vec![
                Ok(Value::SimpleString("request:test".to_string())),
                Ok(Value::Array(vec![
                    Ok(Value::SimpleString("prompt_vec".to_string())),
                    Ok(Value::BulkString(embedding_bytes.clone().into())),
                    Ok(Value::SimpleString("encoded_request".to_string())),
                    Ok(Value::SimpleString(request_json)),
                    Ok(Value::SimpleString("encoded_response".to_string())),
                    Ok(Value::SimpleString(response_json)),
                ])),
            ])),
        ]);

        let parsed = ValkeySearchResult::from_value(&payload).unwrap();
        assert_eq!(parsed.total_results, 1);
        assert_eq!(parsed.documents.len(), 1);
        assert_eq!(parsed.documents[0].id, "request:test");
        assert!(parsed.documents[0].request.is_some());
        assert!(parsed.documents[0].response.is_some());
        assert_eq!(
            parsed.documents[0].embedding.as_deref(),
            Some(embedding.as_slice())
        );
    }
}
