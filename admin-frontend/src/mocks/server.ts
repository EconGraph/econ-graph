/**
 * MSW server setup for tests
 * Based on MSW examples: https://github.com/mswjs/examples/blob/main/examples/with-jest/
 */

import { setupServer } from "msw/node";
import { handlers } from "./handlers";

// This configures a request mocking server with the given request handlers.
export const server = setupServer(...handlers);
