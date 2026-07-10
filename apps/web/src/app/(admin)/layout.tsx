import type { Metadata } from "next";

import { adminBodyFont, adminDataFont, adminDisplayFont } from "./fonts";
import { AdminFooter } from "@/components/admin/AdminFooter";
import { AdminProviders } from "@/components/admin/AdminProviders";
import { AdminSidebar } from "@/components/admin/AdminSidebar";
import { Masthead } from "@/components/admin/Masthead";
import { SentinelTicker } from "@/components/admin/SentinelTicker";
import { AtmosphereOverlay } from "@/components/admin/AtmosphereOverlay";

import "./admin.css";

// The admin dashboard is an internal operator surface (goal 091): kept out
// of every sitemap and explicitly noindexed, same posture as the reviewer
// console.
export const metadata: Metadata = {
  title: {
    default: "Overview",
    template: "%s · govfolio admin",
  },
  robots: { index: false, follow: false },
};

// (admin)'s own root layout (goal 094): a true second Next.js root, with
// its own <html><body> and its own self-hosted Google Fonts (fonts.ts),
// distinct from the public site's system-font stacks. Shell order matches
// the design frame (dc.html:33-103, 1241-1244, 1306-1310): Masthead ->
// SentinelTicker -> a sidebar+main row that flexes to fill -> footer, with
// the atmosphere layers (vignette + grain) LAST in <body> so they sit on
// top of everything. AdminProviders wraps the shell since SentinelTicker
// (and potentially page-level components under `children`) poll through
// its one shared QueryClient.
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <html
      lang="en"
      className={`${adminDisplayFont.variable} ${adminBodyFont.variable} ${adminDataFont.variable}`}
    >
      <body className="admin-root">
        <AdminProviders>
          <Masthead />
          <SentinelTicker />
          <div className="flex min-h-0 flex-1 items-stretch">
            <AdminSidebar />
            <main className="min-w-0 flex-1" style={{ padding: "var(--adm-page-pad)" }}>
              <div style={{ maxWidth: "var(--adm-content-max)", margin: "0 auto" }}>
                {children}
              </div>
            </main>
          </div>
          <AdminFooter />
        </AdminProviders>
        <AtmosphereOverlay />
      </body>
    </html>
  );
}
