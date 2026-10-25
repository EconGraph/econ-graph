/**
 * Validates every GraphQL operation in the frontend against the backend schema.
 *
 * The schema is the SDL snapshot the backend commits at
 * `backend/crates/econ-graph-graphql/schema.graphql` (its `schema_snapshot`
 * test keeps it current). An operation asking for a field, argument or type the
 * backend doesn't serve fails here instead of at runtime.
 *
 * Every `.ts`/`.tsx` file under `src/` is parsed with the TypeScript compiler,
 * except tests, `__mocks__`, stories and setup files. `test-utils/` is scanned
 * on purpose: production financial components import their queries from
 * `test-utils/mocks/graphql`. Each string or template literal (plain or `gql`
 * tagged) whose text starts with `query`, `mutation` or `subscription`, after
 * optional `#` comment lines, and has a selection set is parsed as GraphQL and must
 * hold exactly one named operation. Anonymous `{ ... }` shorthand queries are
 * not detected; name every operation. Known failures live in
 * `graphqlOperationSkipList.ts`.
 *
 * Every operation must also be classified in `utils/graphql.ts`, as a data operation (sent to
 * `VITE_DATA_GRAPHQL_URL` without a token) or an app operation, but not both.
 *
 * Run with `DATA_SCHEMA_REF=<git ref>`, the data operations are also validated against
 * `schema.graphql` as committed at that ref: the data plane a branch reads may run an older
 * release than the branch (ECO-427). Unset, that check is skipped.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {
  buildSchema,
  GraphQLError,
  Kind,
  parse,
  validate,
  type DocumentNode,
  type GraphQLSchema,
  type OperationDefinitionNode,
} from 'graphql';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';
import { APP_OPERATIONS, DATA_OPERATIONS } from '../utils/graphql';
import { GRAPHQL_OPERATION_SKIP_LIST } from './graphqlOperationSkipList';

const SRC_DIR = path.resolve(import.meta.dirname, '..');
const REPO_ROOT = path.resolve(import.meta.dirname, '../../..');
/** The schema snapshot, relative to the repository root. */
const SCHEMA_REPO_PATH = 'backend/crates/econ-graph-graphql/schema.graphql';
const SCHEMA_PATH = path.join(REPO_ROOT, SCHEMA_REPO_PATH);
/** A git ref whose schema the data operations must also validate against, if set. */
const DATA_SCHEMA_REF = process.env.DATA_SCHEMA_REF;

/** Test-only files; they may hold made-up operations for mocks. */
const TEST_FILE = /(^|\/)__tests__\/|(^|\/)__mocks__\/|\.(test|spec|stories)\.tsx?$|(^|\/)setupTests[^/]*$/;

/** Literal text that starts like a GraphQL operation. */
const GRAPHQL_START = /^\s*(#[^\n]*\n\s*)*(query|mutation|subscription)\b\s*[A-Za-z_({@]/;

/**
 * GraphQL-looking text that also has a selection set (`{`), so prose such as
 * "query failed" is not mistaken for an operation.
 */
function looksLikeGraphQL(text: string): boolean {
  return GRAPHQL_START.test(text) && text.includes('{');
}

/** Number of operations the scan finds today; see the count test below. */
const MIN_OPERATIONS = 43;

interface FoundOperation {
  /** `<path under src/>#<OperationName>`, the skip list key. */
  key: string;
  /** Validation or parse errors; empty when the operation is valid. */
  errors: string[];
  /** The operation's name, type and parsed document, when the literal held one named operation. */
  operation?: { name: string; type: OperationDefinitionNode['operation']; document: DocumentNode };
}

/** Non-test `.ts`/`.tsx` files under `src/`, as posix paths relative to it. */
function sourceFiles(): string[] {
  return (fs.readdirSync(SRC_DIR, { recursive: true }) as string[])
    .map(file => file.split(path.sep).join('/'))
    .filter(file => /\.tsx?$/.test(file) && !file.endsWith('.d.ts') && !TEST_FILE.test(file))
    .sort();
}

interface GraphQLLiteral {
  line: number;
  /** Literal text, or null for a template with `${...}` substitutions. */
  text: string | null;
}

/** String and template literals in a source file that start like GraphQL. */
function graphqlLiterals(file: string, source: string): GraphQLLiteral[] {
  const sourceFile = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  const literals: GraphQLLiteral[] = [];
  /** 1-based line of a node's start. */
  const line = (node: ts.Node) =>
    sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;

  /** Collects GraphQL-looking literals from `node` and its descendants. */
  const visit = (node: ts.Node) => {
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
      if (looksLikeGraphQL(node.text)) literals.push({ line: line(node), text: node.text });
    } else if (ts.isTemplateExpression(node)) {
      const staticText = [node.head.text, ...node.templateSpans.map(span => span.literal.text)].join('');
      if (looksLikeGraphQL(staticText)) literals.push({ line: line(node), text: null });
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return literals;
}

/** Errors graphql-js `validate` leaves to execution: a missing root type. */
function rootTypeErrors(schema: GraphQLSchema, document: DocumentNode): string[] {
  return document.definitions
    .filter((def): def is OperationDefinitionNode => def.kind === Kind.OPERATION_DEFINITION)
    .filter(op => !schema.getRootType(op.operation))
    .map(op => `Schema has no ${op.operation} root type.`);
}

/** Every GraphQL operation in `src/`, with its validation errors against `schema`. */
function findOperations(schema: GraphQLSchema): FoundOperation[] {
  const found: FoundOperation[] = [];
  for (const file of sourceFiles()) {
    const source = fs.readFileSync(path.join(SRC_DIR, file), 'utf8');
    for (const { line, text } of graphqlLiterals(file, source)) {
      if (text === null) {
        found.push({
          key: `${file}:${line}`,
          errors: ['GraphQL template has ${...} substitutions; the scan cannot validate it.'],
        });
        continue;
      }

      let document: DocumentNode;
      try {
        document = parse(text);
      } catch (error) {
        const message = error instanceof GraphQLError ? error.message : String(error);
        found.push({ key: `${file}:${line}`, errors: [`Parse error: ${message}`] });
        continue;
      }

      const operations = document.definitions.filter(
        (def): def is OperationDefinitionNode => def.kind === Kind.OPERATION_DEFINITION
      );
      if (operations.length !== 1 || !operations[0].name) {
        found.push({
          key: `${file}:${line}`,
          errors: ['Expected exactly one named operation in this GraphQL literal.'],
        });
        continue;
      }

      const errors = [
        ...validate(schema, document).map(error => error.message),
        ...rootTypeErrors(schema, document),
      ];
      const name = operations[0].name.value;
      found.push({
        key: `${file}#${name}`,
        errors,
        operation: { name, type: operations[0].operation, document },
      });
    }
  }
  return found;
}

const schema = buildSchema(fs.readFileSync(SCHEMA_PATH, 'utf8'));
const operations = findOperations(schema);
const skipped = new Set(Object.keys(GRAPHQL_OPERATION_SKIP_LIST));

describe('frontend GraphQL operations', () => {
  it('finds the operations in src/', () => {
    // Guards against the scan silently missing operations. Lower
    // MIN_OPERATIONS when a PR deletes operations.
    const keys = operations.map(op => op.key);
    expect(keys).toContain('utils/graphql.ts#GetSeriesDetail');
    expect(keys).toContain('utils/graphql.ts#TriggerCrawl');
    expect(operations.length).toBeGreaterThanOrEqual(MIN_OPERATIONS);
  });

  it('has no duplicate operation keys', () => {
    const keys = operations.map(op => op.key);
    expect(keys.filter((key, i) => keys.indexOf(key) !== i)).toEqual([]);
  });

  it.each(operations.filter(op => !skipped.has(op.key)).map(op => [op.key, op]))(
    '%s validates against schema.graphql',
    (_key, op) => {
      expect((op as FoundOperation).errors).toEqual([]);
    }
  );

  it.each(operations.filter(op => skipped.has(op.key)).map(op => [op.key, op]))(
    '%s is on the skip list and still fails (remove the entry once it validates)',
    (_key, op) => {
      expect((op as FoundOperation).errors).not.toEqual([]);
    }
  );

  it('has no skip list entries for operations that no longer exist', () => {
    const keys = new Set(operations.map(op => op.key));
    expect([...skipped].filter(key => !keys.has(key))).toEqual([]);
  });
});

describe('frontend GraphQL operation routing', () => {
  const named = operations.flatMap(op => (op.operation ? [op.operation] : []));
  const names = new Set(named.map(op => op.name));

  it.each([...names].sort().map(name => [name]))(
    '%s is classified as exactly one of data or app (utils/graphql.ts)',
    name => {
      const classes = [DATA_OPERATIONS.has(name) && 'data', APP_OPERATIONS.has(name) && 'app'];
      expect(classes.filter(Boolean)).toHaveLength(1);
    }
  );

  it('classifies only operations that exist', () => {
    const classified = [...DATA_OPERATIONS, ...APP_OPERATIONS];
    expect(classified.filter(name => !names.has(name))).toEqual([]);
  });

  it('has only queries among the data operations', () => {
    const notQueries = named.filter(op => DATA_OPERATIONS.has(op.name) && op.type !== 'query');
    expect(notQueries.map(op => op.name)).toEqual([]);
  });
});

describe.skipIf(!DATA_SCHEMA_REF)(
  `data operations against schema.graphql at DATA_SCHEMA_REF${
    DATA_SCHEMA_REF ? ` (${DATA_SCHEMA_REF})` : ' (skipped: DATA_SCHEMA_REF is unset)'
  }`,
  () => {
    // Skip-listed operations already fail against today's schema; they are checked above.
    const dataOperations = operations.filter(
      op => op.operation && DATA_OPERATIONS.has(op.operation.name) && !skipped.has(op.key)
    );

    /** The schema committed at DATA_SCHEMA_REF; throws if git can't read it. */
    const refSchema = (): GraphQLSchema => {
      if (DATA_SCHEMA_REF?.startsWith('-')) {
        throw new Error(`DATA_SCHEMA_REF must be a git ref, not an option: ${DATA_SCHEMA_REF}`);
      }
      return buildSchema(
        // --end-of-options as well: nothing after it is read as a git option.
        execFileSync('git', ['show', '--end-of-options', `${DATA_SCHEMA_REF}:${SCHEMA_REPO_PATH}`], {
          cwd: REPO_ROOT,
          encoding: 'utf8',
          stdio: ['ignore', 'pipe', 'pipe'],
        })
      );
    };
    let schemaAtRef: GraphQLSchema | undefined;

    it('reads schema.graphql at the ref', () => {
      schemaAtRef = refSchema();
      expect(schemaAtRef.getQueryType()).toBeTruthy();
    });

    it.each(dataOperations.map(op => [op.key, op]))('%s validates at the ref', (_key, op) => {
      schemaAtRef ??= refSchema();
      const { document } = (op as FoundOperation).operation!;
      const errors = [
        ...validate(schemaAtRef, document).map(error => error.message),
        ...rootTypeErrors(schemaAtRef, document),
      ];
      expect(errors).toEqual([]);
    });
  }
);

if (!DATA_SCHEMA_REF) {
  console.info(
    'graphql-operations: the data operations were not checked against an older schema; ' +
      'set DATA_SCHEMA_REF=<git ref> to run that check.'
  );
}
