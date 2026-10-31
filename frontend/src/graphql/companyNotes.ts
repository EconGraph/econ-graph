import { executeGraphQL } from '../utils/graphql';

export interface CompanyNote {
  id: string;
  anchor: string;
  cik: string;
  content: string;
  createdAt: string;
  updatedAt: string;
  targetStatus: 'found' | 'not_found' | 'unavailable';
}

const FIELDS = 'id anchor cik content createdAt updatedAt targetStatus';

/**
 * Build the common versioned company target.
 * @param cik - Canonical ten-digit issuer CIK.
 * @returns The annotation-anchor JSON.
 */
export function companyNoteAnchor(cik: string): string {
  return JSON.stringify({
    version: '1',
    target: { kind: 'company', cik },
    chart_id: null,
    observation_date: null,
    observation_value: null,
  });
}

/**
 * Read a page of the signed-in user's company notes.
 * @param cik - Canonical issuer CIK.
 * @param anchor - Optional immutable target filter.
 * @param offset - Number of preceding notes.
 * @returns Up to 100 private notes.
 */
export async function getCompanyNotes(
  cik: string,
  anchor?: string,
  offset = 0
): Promise<CompanyNote[]> {
  const result = await executeGraphQL<{ companyNotes: CompanyNote[] }>({
    query: `query CompanyNotes($cik: String!, $anchor: String, $offset: Int) { companyNotes(cik: $cik, anchor: $anchor, offset: $offset) { ${FIELDS} } }`,
    variables: { cik, anchor, offset },
  });
  if (!result.data) throw new Error('Company notes could not be loaded');
  return result.data.companyNotes;
}

/**
 * Save a private note after the backend checks target access.
 * @param anchor - Versioned target JSON.
 * @param content - User-authored note.
 * @returns The saved note.
 */
export async function createCompanyNote(anchor: string, content: string): Promise<CompanyNote> {
  const result = await executeGraphQL<{ createCompanyNote: CompanyNote }>({
    query: `mutation CreateCompanyNote($anchor: String!, $content: String!) { createCompanyNote(anchor: $anchor, content: $content) { ${FIELDS} } }`,
    variables: { anchor, content },
  });
  if (!result.data) throw new Error('Company note could not be saved');
  return result.data.createCompanyNote;
}

/**
 * Update a note owned by the signed-in user.
 * @param id - Note ID.
 * @param content - Replacement content.
 * @returns The saved note.
 */
export async function updateCompanyNote(id: string, content: string): Promise<CompanyNote> {
  const result = await executeGraphQL<{ updateCompanyNote: CompanyNote }>({
    query: `mutation UpdateCompanyNote($id: ID!, $content: String!) { updateCompanyNote(id: $id, content: $content) { ${FIELDS} } }`,
    variables: { id, content },
  });
  if (!result.data) throw new Error('Company note could not be updated');
  return result.data.updateCompanyNote;
}

/**
 * Delete a note owned by the signed-in user.
 * @param id - Note ID.
 * @returns True when the note was deleted.
 */
export async function deleteCompanyNote(id: string): Promise<boolean> {
  const result = await executeGraphQL<{ deleteCompanyNote: boolean }>({
    query: 'mutation DeleteCompanyNote($id: ID!) { deleteCompanyNote(id: $id) }',
    variables: { id },
  });
  if (!result.data?.deleteCompanyNote) throw new Error('Company note could not be deleted');
  return true;
}
