#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const testFile = path.join(__dirname, 'src/pages/__tests__/CrawlerConfig.test.tsx');

let content = fs.readFileSync(testFile, 'utf8');

// Fix remaining await user.click() calls
content = content.replace(/await user\.click\(/g, 'fireEvent.click(');

// Fix remaining await user.type() calls
content = content.replace(/await user\.type\(/g, 'fireEvent.change(');

// Fix user.type() calls to use proper fireEvent.change syntax
content = content.replace(/fireEvent\.change\(([^,]+), "([^"]+)"\)/g, 'fireEvent.change($1, { target: { value: "$2" } })');

// Remove remaining await keywords that are now invalid
content = content.replace(/await fireEvent\./g, 'fireEvent.');

fs.writeFileSync(testFile, content);

console.log('Fixed remaining issues in CrawlerConfig tests');
