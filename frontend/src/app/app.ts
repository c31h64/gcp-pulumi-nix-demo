import { Component, signal, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { AdjudicateOutcome } from '../types';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [],
  templateUrl: './app.html',
  styleUrl: './app.css'
})
export class App {
  message = signal<string>('');

  has_outcome = signal<boolean>(false);
  arguments_side_a = signal<string>('');
  arguments_side_b = signal<string>('');
  winner_side = signal<string>('');
  winner_probability = signal<number>(-1.0);
  compromise_solution = signal<string>('');

  private http = inject(HttpClient);

  adjudicate() {
    this.http.post<AdjudicateOutcome>('/adjudicate', {
      'problem_text':  document.getElementById("problemStatementWidget")?.textContent,
      'side_a_text': document.getElementById("sideAWidget")?.textContent,
      'side_b_text': document.getElementById("sideBWidget")?.textContent,
    }).subscribe({
      next: (res: AdjudicateOutcome) => {
        this.has_outcome.set(true);
        this.arguments_side_a.set(res.arguments_side_a);
        this.arguments_side_b.set(res.arguments_side_b);
        this.winner_side.set(res.winner_side);
        this.winner_probability.set(res.winner_probability);
        this.compromise_solution.set(res.compromise_solution);
        this.message.set(JSON.stringify(res, null, 2));
      },
      error: (err) => {
        console.error(err);
        this.message.set('Error calling the adjudicate API');
      }
    });
  }

  fetchData() {
     this.http
       .get('/quote', { responseType: 'text' })
       .subscribe({
          next: (res: string) => {
             this.message.set(res);
          },
          error: (err) => {
            console.error(err);
            this.message.set('Error fetching quote');
          }
       });
  }
}
