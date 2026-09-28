/**
 * Every in-app path the frontend navigates to must be one a route in `src/App.tsx` serves.
 *
 * Every `.ts`/`.tsx` file under `src/` is parsed with the TypeScript compiler, except tests,
 * `__mocks__`, stories and setup files. A navigation target is a string or template literal that
 * starts with `/` and is:
 * - the first argument of `navigate(...)`, `location.assign(...)` or `location.replace(...)`;
 * - assigned to `location.href` (`window.location.href = '/…'`);
 * - the value of an `href` or `to` JSX attribute;
 * - the value of a `path`, `href` or `to` property in an object literal (the sidebar's menu).
 * A `${...}` that fills a whole path segment matches one segment, so `/series/${id}` matches
 * `/series/:id`; the path is only checked up to a `${...}` glued to other text. Anything after `?`
 * or `#` is ignored, and so are paths to files (`/manifest.json`) and to the API (`/api/…`,
 * `/graphql`). Targets built any other way are not checked; the release crawl (REL-4) is the
 * backstop for those.
 *
 * Known dead targets live in KNOWN_DEAD below, each with the issue that fixes it. An entry whose
 * target no longer appears fails the test, so the list stays exact; one whose file is gone is
 * ignored, so the PR that deletes a page doesn't have to edit this file.
 */
import fs from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import * as oidcConfig from '../auth/oidcConfig';

const SRC_DIR = path.resolve(import.meta.dirname, '..');

/** Test-only files; they may navigate anywhere. */
const TEST_FILE =
  /(^|\/)__tests__\/|(^|\/)__mocks__\/|\.(test|spec|stories)\.tsx?$|(^|\/)setupTests[^/]*$/;

/** Stands in for a template literal's `${...}` part. */
const PARAM = ':param';

/**
 * Targets that go nowhere today, keyed `<path under src/>#<target>`, with the issue that fixes
 * each. Remove an entry when its fix lands.
 */
const KNOWN_DEAD: Record<string, string> = {
  'pages/ProfessionalAnalysis.tsx#/dashboard':
    'UI-1 (ECO-54, #199) deletes this mock page and its route.',
};

/**
 * Lists the source files to scan.
 * @param dir - Directory to walk.
 * @returns Paths relative to `src/`, with `/` separators.
 */
function sourceFiles(dir: string = SRC_DIR): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === 'node_modules' ? [] : sourceFiles(full);
    const rel = path.relative(SRC_DIR, full).split(path.sep).join('/');
    return /\.tsx?$/.test(entry.name) && !entry.name.endsWith('.d.ts') && !TEST_FILE.test(rel)
      ? [rel]
      : [];
  });
}

/**
 * The text of a string or template literal, with each `${...}` replaced by one path segment.
 * @param node - Any expression.
 * @returns The text, or null when the node is not a literal.
 */
function literalText(node: ts.Node): string | null {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
  if (ts.isTemplateExpression(node)) {
    return node.head.text + node.templateSpans.map(span => PARAM + span.literal.text).join('');
  }
  return null;
}

/** Paths that are not pages: the API, and files such as `/manifest.json`. */
const NOT_A_PAGE = /^\/(api|graphql)(\/|$)|\.[a-z0-9]+$/i;

/**
 * Whether an expression is `location.href`, `window.location.href` or `document.location.href`.
 * @param node - The left side of an assignment.
 * @returns True for a `.href` on something named `location`.
 */
const isLocationHref = (node: ts.Node) =>
  ts.isPropertyAccessExpression(node) &&
  node.name.text === 'href' &&
  (ts.isIdentifier(node.expression)
    ? node.expression.text === 'location'
    : ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === 'location');

/** Property and attribute names whose value is a navigation target. */
const TARGET_NAMES = new Set(['href', 'to', 'path']);

interface Target {
  /** Path under `src/`. */
  file: string;
  line: number;
  /** The target as written, with `${...}` parts as `:param`. */
  target: string;
}

/**
 * Collects the in-app navigation targets in one file.
 * @param file - Path under `src/`.
 * @returns The targets that start with a single `/`.
 */
function targetsIn(file: string): Target[] {
  const text = fs.readFileSync(path.join(SRC_DIR, file), 'utf8');
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const found: Target[] = [];
  const add = (node: ts.Node | undefined) => {
    if (!node) return;
    const value = literalText(node);
    if (value === null || !value.startsWith('/') || value.startsWith('//')) return;
    if (NOT_A_PAGE.test(value.split(/[?#]/)[0])) return;
    const { line } = source.getLineAndCharacterOfPosition(node.getStart());
    found.push({ file, line: line + 1, target: value });
  };
  const visit = (node: ts.Node) => {
    if (
      ts.isCallExpression(node) &&
      ((ts.isIdentifier(node.expression) && node.expression.text === 'navigate') ||
        (ts.isPropertyAccessExpression(node.expression) &&
          ['assign', 'replace'].includes(node.expression.name.text) &&
          node.expression.expression.getText(source).endsWith('location')))
    ) {
      add(node.arguments[0]);
    } else if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      isLocationHref(node.left)
    ) {
      add(node.right);
    } else if (ts.isJsxAttribute(node) && ts.isIdentifier(node.name)) {
      // `path` on a JSX element is a <Route>'s own pattern, not a target.
      if (node.name.text !== 'path' && TARGET_NAMES.has(node.name.text) && node.initializer) {
        add(
          ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer
        );
      }
    } else if (
      ts.isPropertyAssignment(node) &&
      ts.isIdentifier(node.name) &&
      TARGET_NAMES.has(node.name.text)
    ) {
      add(node.initializer);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return found;
}

/**
 * The `path` of every `<Route>` in App.tsx: a string literal, or a constant exported by
 * `auth/oidcConfig`.
 * @returns The paths, in declaration order.
 */
function appRoutes(): string[] {
  const text = fs.readFileSync(path.join(SRC_DIR, 'App.tsx'), 'utf8');
  const source = ts.createSourceFile('App.tsx', text, ts.ScriptTarget.Latest, true);
  const routes: string[] = [];
  const visit = (node: ts.Node) => {
    if (
      (ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) &&
      node.tagName.getText(source) === 'Route'
    ) {
      const attr = node.attributes.properties.find(
        (p): p is ts.JsxAttribute =>
          ts.isJsxAttribute(p) && ts.isIdentifier(p.name) && p.name.text === 'path'
      );
      const init = attr?.initializer;
      const expr = init && ts.isJsxExpression(init) ? init.expression : init;
      const value =
        expr && ts.isIdentifier(expr)
          ? (oidcConfig as Record<string, unknown>)[expr.text]
          : expr && literalText(expr);
      if (typeof value !== 'string') {
        throw new Error(`App.tsx: a <Route> path this test can't read: ${node.getText(source)}`);
      }
      routes.push(value);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return routes;
}

/**
 * A route path as a regular expression: `:id` is one segment, `:id?` or `about?` an optional one,
 * and `*` any rest of the path.
 * @param route - The path as React Router reads it.
 * @returns The anchored pattern, allowing a trailing slash.
 */
function routePattern(route: string): RegExp {
  const body = route
    .split('/')
    .filter(Boolean)
    .map(seg =>
      seg === '*'
        ? '(?:/.*)?'
        : seg.startsWith(':')
          ? seg.endsWith('?')
            ? '(?:/[^/]+)?'
            : '/[^/]+'
          : seg.endsWith('?')
            ? `(?:/${escape(seg.slice(0, -1))})?`
            : `/${escape(seg)}`
    )
    .join('');
  return new RegExp(`^${body}/?$`);
}

/**
 * Escapes text for use in a regular expression.
 * @param text - A static path segment.
 * @returns The escaped text.
 */
function escape(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

const routes = appRoutes();
const patterns = routes.map(routePattern);
const targets = sourceFiles().flatMap(targetsIn);

/**
 * Whether a target is served by some route.
 * @param target - As collected, possibly with a query or fragment.
 * @returns True when its pathname matches a route.
 */
const served = (target: string) => {
  const pathname = target.split(/[?#]/)[0] || '/';
  const segments = pathname.split('/');
  // A `${...}` glued to other text (`/explore${qs}`) could hold anything, even more segments, so
  // only the text before it is checked, as the start of some route's path.
  const glued = segments.findIndex(seg => seg.includes(PARAM) && seg !== PARAM);
  if (glued === -1) return patterns.some(p => p.test(pathname));
  const prefix = [
    ...segments.slice(0, glued),
    segments[glued].slice(0, segments[glued].indexOf(PARAM)),
  ].join('/');
  return routes.some(
    (route, i) => route.startsWith(prefix) || patterns[i].test(prefix.replace(/\/$/, '') || '/')
  );
};

describe('navigation targets', () => {
  it('finds the routes and the targets it checks', () => {
    // Guards against the scan silently finding nothing after a refactor.
    expect(routes.length).toBeGreaterThanOrEqual(10);
    expect(targets.length).toBeGreaterThanOrEqual(25);
  });

  it('matches targets against routes as React Router would', () => {
    expect(served('/series/:param')).toBe(true);
    expect(served('/analysis')).toBe(true);
    expect(served('/explore?q=:param')).toBe(true);
    expect(served('/explore:param')).toBe(true);
    expect(served('/series/:param/:param')).toBe(false);
    expect(served('/dashboard')).toBe(false);
    expect(served('/dash:param')).toBe(false);
  });

  it('every in-app navigation target is served by a route in App.tsx', () => {
    const dead = targets
      .filter(t => !served(t.target) && !(`${t.file}#${t.target}` in KNOWN_DEAD))
      .map(t => `${t.file}:${t.line} ${t.target}`);
    expect(dead, 'targets no <Route> in src/App.tsx serves').toEqual([]);
  });

  it('every KNOWN_DEAD entry is still dead, and names a fix', () => {
    const stale = Object.entries(KNOWN_DEAD)
      .filter(([key, reason]) => {
        expect(reason.trim().length, key).toBeGreaterThan(10);
        const [file, target] = [key.slice(0, key.indexOf('#')), key.slice(key.indexOf('#') + 1)];
        if (!fs.existsSync(path.join(SRC_DIR, file))) return false;
        return !targets.some(t => t.file === file && t.target === target && !served(target));
      })
      .map(([key]) => key);
    expect(stale, 'KNOWN_DEAD entries to delete: the target is gone or now served').toEqual([]);
  });
});
