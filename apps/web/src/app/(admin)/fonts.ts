// govfolio admin — self-hosted Google Fonts (goal 094).
//
// The admin console gets its own type system, distinct from the public
// site's system-font stacks (see (site)/globals.css). Each loader exposes
// its generated font-family via the `variable` CSS custom property, applied
// on (admin)'s root <html> element (Task 5). admin.css references these
// variables with the public site's font stacks as the CSS fallback chain.
import { IBM_Plex_Mono, Libre_Baskerville, Public_Sans } from "next/font/google";

export const adminDisplayFont = Libre_Baskerville({
  weight: ["400", "700"],
  style: ["normal", "italic"],
  subsets: ["latin"],
  display: "swap",
  variable: "--adm-font-display-family",
});

export const adminBodyFont = Public_Sans({
  weight: ["400", "500", "600", "700"],
  subsets: ["latin"],
  display: "swap",
  variable: "--adm-font-body-family",
});

export const adminDataFont = IBM_Plex_Mono({
  weight: ["400", "500", "600"],
  subsets: ["latin"],
  display: "swap",
  variable: "--adm-font-data-family",
});
