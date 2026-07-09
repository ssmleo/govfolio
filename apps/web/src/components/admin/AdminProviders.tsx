"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";

// The ONLY place TanStack Query is wired into this app (goal 091): one
// shared `QueryClient` per browser session, instantiated once via
// `useState` so client-side navigations reuse it instead of resetting the
// cache. Every admin page/component that polls the API (StatusStrip, and
// any page-level `useQuery` the page-building agents add) reads through
// this provider.
export function AdminProviders({ children }: { children: React.ReactNode }) {
  const [client] = useState(() => new QueryClient());
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
