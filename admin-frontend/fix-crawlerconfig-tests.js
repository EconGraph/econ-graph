#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const testFile = path.join(
  __dirname,
  "src/pages/__tests__/CrawlerConfig.test.tsx",
);

let content = fs.readFileSync(testFile, "utf8");

// Replace user.click() with fireEvent.click() for Switch components
content = content.replace(
  /await user\.click\(([^)]+)\);/g,
  "fireEvent.click($1);",
);

// Replace user.type() with fireEvent.change() for TextField components
content = content.replace(
  /await user\.clear\(([^)]+)\);\s*await user\.type\(([^)]+), "([^"]+)"\);/g,
  'fireEvent.change($1, { target: { value: "$3" } });',
);

// Remove userEvent.setup() calls since we're not using user interactions
content = content.replace(/const user = userEvent\.setup\(\);\s*/g, "");

// Remove async from test functions that no longer need it
content = content.replace(
  /it\("([^"]+)", async \(\) => \{/g,
  'it("$1", () => {',
);

fs.writeFileSync(testFile, content);

console.log(
  "Fixed CrawlerConfig tests by replacing user interactions with fireEvent alternatives",
);
