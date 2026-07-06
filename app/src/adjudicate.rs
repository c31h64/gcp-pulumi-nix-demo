use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use threatflux_vertex_rust_sdk::builders::{ContentRequestBuilder, FunctionBuilder};
use threatflux_vertex_rust_sdk::{FunctionDeclaration, GenerateContentRequest, Tool, VertexClient};

use crate::MODEL_NAME;

// Build the function using the ergonomic FunctionBuilder
fn create_adjudicate_fn() -> FunctionDeclaration {
    FunctionBuilder::new("adjudicate", "Return the winning arguments from side A in a list. Same for side B. Suggest compromise solution and answer who is right, A or B and provide a probability metric.")
    .parameter("arguments_side_a", "string", "Arguments supporting that side A is the right one!")
    .parameter("arguments_side_b", "string", "Arguments supporting that side B is the right one!")
    .enum_parameter("winner_side",  "string", "Indicator of A or B telling us which side is the winner!", vec!["A", "B"])
    .number_parameter("winner_probability", "Confidence that the winner is indeed the chosen side!", Some(0.0), Some(1.0))
    .parameter("compromise_solution", "string", "Compromise solution to the stated problem if one is possible. Return empty text here if no compromise is possible!")
    .required_parameters(vec![
        "arguments_side_a", 
        "arguments_side_b", 
        "winner_side", 
        "winner_probability", 
        "compromise_solution"
    ])
    .build()
}

fn adjudicate_tool() -> Tool {
    Tool::FunctionCalling {
        function_declarations: vec![create_adjudicate_fn()],
    }
}

#[derive(Deserialize)]
pub struct AdjudicateRequest {
    problem_text: String,
    side_a_text: String,
    side_b_text: String,
}

fn build_adjudicate_gemini_request(request: AdjudicateRequest) -> GenerateContentRequest {
    let base_msg = "Adjudicate between two sides A and B and use the adjudicate function to provide your answer. Be as impartial as possible.";
    let prompt = format!(
        "{}\n Problem: {} \n\n\n Side A: {} \n\n\n Side B: {}",
        base_msg, request.problem_text, request.side_a_text, request.side_b_text
    );
    let builder = ContentRequestBuilder::new(prompt)
        .temperature(0.0)
        .tool(adjudicate_tool())
        .with_thinking();

    builder.build()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdjudicateOutcome {
    arguments_side_a: String,
    arguments_side_b: String,
    winner_side: String,
    winner_probability: f32,
    compromise_solution: String,
}

pub async fn adjudicate(
    client: Arc<VertexClient>,
    request: AdjudicateRequest,
) -> anyhow::Result<AdjudicateOutcome> {
    let request = build_adjudicate_gemini_request(request);
    let response = client.generate_with_functions(MODEL_NAME, &request).await?;

    if let Some(function_call) = response.function_calls().iter().next() {
        return serde_json::from_value(serde_json::to_value(&function_call.args)?)
            .context("Deserialize failure!");
    }

    Err(anyhow::format_err!(
        "No function call was given to us by the LLM!"
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::create_vertex_client;

    #[tokio::test]
    async fn test_adjudicate() {
        let client = Arc::new(create_vertex_client().await.unwrap());
        let request = AdjudicateRequest {
            problem_text: "Violets are red. Roses are blue.".to_string(),
            side_a_text: "Violets are more like purple.".to_string(),
            side_b_text: "Violets are more like burgundy.".to_string(),
        };
        let outcome = adjudicate(client, request).await;

        if let Err(e) = &outcome {
            eprintln!("{:?}", e);
        }

        if let Ok(adj) = &outcome {
            println!("{:?}", adj);
        }

        assert_eq!(outcome.is_ok(), true);
    }
}
