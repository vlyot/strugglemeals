import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
import { AuthView } from "@neondatabase/neon-js/auth/react";
import App from "./App";
import HistoryPage from "./pages/HistoryPage";
import FavouritesPage from "./pages/FavouritesPage";
import CookPage from "./pages/CookPage";
import { authClient } from "./stack/client";

function ProtectedRoute() {
  const { data: session, isPending } = authClient.useSession();
  if (isPending) return null;
  if (!session) return <Navigate to="/handler/sign-in" replace />;
  return <Outlet />;
}

export const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
  },
  {
    path: "/cook",
    element: <CookPage />,
  },
  {
    path: "/handler/:pathParam?",
    element: (
      <div className="min-h-screen flex items-center justify-center bg-background px-4">
        <AuthView />
      </div>
    ),
  },
  {
    element: <ProtectedRoute />,
    children: [
      { path: "/history", element: <HistoryPage /> },
      { path: "/favourites", element: <FavouritesPage /> },
    ],
  },
]);
