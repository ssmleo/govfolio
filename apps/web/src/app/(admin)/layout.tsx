import type { Metadata } from "next";

import { AdminNav } from "@/components/admin/AdminNav";
import { AdminProviders } from "@/components/admin/AdminProviders";
import { StatusStrip } from "@/components/admin/StatusStrip";

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

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="admin-root">
      <AdminNav />
      <AdminProviders>
        <StatusStrip />
        {children}
      </AdminProviders>
    </div>
  );
}
