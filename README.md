# StruggleMeals

Real recipes from whatever you have. Part of the [Patchwork series](https://github.com/vlyot).

## Stack

| Layer | Technology |
|---|---|
| Frontend | React + TypeScript + Vite + shadcn/ui |
| Backend | Rust + Axum |
| Database | Neon (Postgres) |
| Recipe dataset | SQLite (RecipeNLG subset) |
| Auth | Neon Auth (Better Auth) |
| AI | Gemini Vision + Groq |
| Hosting | Vercel (frontend) + Railway (backend) |

## Development

### Backend

```bash
cd backend
cp .env.example .env  # fill in DATABASE_URL
cargo run
```

Health check: `GET http://localhost:8080/health`

### Frontend

```bash
cd frontend
cp .env.example .env  # set VITE_API_URL
npm install
npm run dev
```

## Project structure

```
strugglemeals/
├── backend/       # Rust + Axum API
├── frontend/      # Vite + React + TS
└── .github/
    └── workflows/ # CI
```
