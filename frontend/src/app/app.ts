import { Component, signal, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [],
  templateUrl: './app.html',
  styleUrl: './app.css'
})
export class App {
  message = signal<string>('');
  private http = inject(HttpClient);

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
