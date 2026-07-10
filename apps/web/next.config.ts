import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  experimental: {
    // Two top-level root layouts now exist ((site) and (admin)); there's no
    // single shared layout left to compose a fallback 404 from, so genuinely
    // unmatched paths (not caught by any route group) are served by
    // app/global-not-found.tsx instead. See Next.js docs: file-conventions/not-found.
    globalNotFound: true,
  },
};

export default nextConfig;
