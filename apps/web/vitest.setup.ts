import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// RTL only auto-registers cleanup when test globals exist; we keep vitest
// globals off, so register it explicitly.
afterEach(cleanup);
