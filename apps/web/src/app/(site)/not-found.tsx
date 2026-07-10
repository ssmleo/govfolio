import Link from "next/link";

export default function NotFound() {
  return (
    <section className="profile-head">
      <h1>Not found</h1>
      <p className="muted">
        Nothing is published at this address. It may have been superseded or
        never existed.
      </p>
      <p>
        <Link href="/">Back to the latest records</Link> ·{" "}
        <Link href="/search">Search</Link>
      </p>
    </section>
  );
}
