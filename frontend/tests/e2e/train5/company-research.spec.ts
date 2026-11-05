import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';

/** Real APIs and a pre-authenticated browser; no request interception or synthetic response data. */
test('company research workflow', async ({ page }) => {
  const cik = process.env.TRAIN5_COMPANY_CIK;
  if (!cik || !/^\d{10}$/.test(cik))
    throw new Error('TRAIN5_COMPANY_CIK must identify a loaded ten-digit SEC fixture company.');
  await page.goto('/companies');
  await page.getByRole('textbox', { name: 'Find company' }).fill(cik);
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  const company = page.locator(`a[href="/companies/${cik}"]`);
  await expect(company).toHaveCount(1);
  const companyName = await company.innerText();
  await company.click();
  await expect(page.getByRole('heading', { name: companyName, exact: true })).toBeVisible();
  const kinds = ['Income statement', 'Balance sheet', 'Cash flow'];
  for (const kind of kinds) {
    await page.getByRole('combobox', { name: 'Statement', exact: true }).click();
    await page.getByRole('option', { name: kind, exact: true }).click();
    await expect(page.getByRole('table', { name: `${kind} values` })).toBeVisible();
    await expect(page.getByRole('combobox', { name: 'Period', exact: true })).toBeVisible();
    const picker = page.getByRole('combobox', { name: 'Period', exact: true });
    const firstPeriod = await picker.innerText();
    await picker.click();
    await expect(page.locator('[role=option][data-statement-period=true]').nth(1)).toBeVisible();
    await page.locator('[role=option][data-statement-period=true]').nth(1).click();
    await expect(picker).not.toHaveText(firstPeriod);
    const periodLabel = await picker.innerText();
    const dates = periodLabel.match(/\d{4}-\d{2}-\d{2}/g)!;
    const periodStart = dates.length === 2 ? dates[0] : '';
    const periodEnd = dates[dates.length - 1];
    const table = page.getByRole('table', { name: `${kind} values` });
    const row = table.locator('tbody tr').first();
    await expect(row).toBeVisible();
    const exactValue = await row.locator('td').nth(1).innerText();
    const unit = await row.locator('td').nth(2).innerText();
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('button', { name: 'Download statement CSV', exact: true }).click(),
    ]);
    const csv = readFileSync(await download.path(), 'utf8');
    expect(csv).toContain('U.S. Securities and Exchange Commission (SEC)');
    expect(csv).toContain(
      'taxonomy,concept,label,unit,period_start,period_end,value,accession,filed,derivation,source_accessions'
    );
    const label = await row.getByRole('button', { name: /^Chart / }).innerText();
    const conceptText = await row.getByTestId('company-concept').innerText();
    const [taxonomy, concept] = conceptText.split(':');
    const exported = csvRows(csv).filter(
      record =>
        record[0] === taxonomy &&
        record[1] === concept &&
        record[4] === periodStart &&
        record[5] === periodEnd
    );
    expect(exported).toContainEqual(expect.arrayContaining([label, unit, exactValue]));
    const exactRow = exported.find(
      record => record[2] === label && record[3] === unit && record[6] === exactValue
    )!;
    expect(exactRow).toBeTruthy();
    for (const accession of await row.locator('a[href*="/Archives/"]').allTextContents()) {
      expect(exactRow[10].split(';')).toContain(accession);
    }
    expect(csv).toContain('Chart URL,');
    await row.getByRole('button', { name: /^Chart / }).click();
    await expect(page.getByRole('img', { name: /line chart$/ })).toBeVisible();
    const [lineDownload] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('button', { name: 'Download line CSV', exact: true }).click(),
    ]);
    expect(csvRows(readFileSync(await lineDownload.path(), 'utf8'))).toContainEqual(exactRow);
  }
  const comparisons = page.getByRole('region', { name: 'Ratios and peers' });
  await expect(comparisons.getByRole('table', { name: 'Financial ratios' }).first()).toBeVisible();
  await expect(comparisons).not.toContainText('Ratios and peers could not be loaded');
  // The loaded fixed corpus must include at least one SIC peer for this end-to-end fixture.
  await expect(comparisons.locator('a[href^="/companies/"]').first()).toBeVisible();
  await page.getByRole('combobox', { name: 'Statement', exact: true }).click();
  await page.getByRole('option', { name: 'Income statement', exact: true }).click();
  await page
    .getByRole('button', { name: /^Chart / })
    .first()
    .click();
  const notes = page.getByRole('region', { name: 'My company notes' });
  await expect(notes).toBeVisible();
  const original = `Train 5 evidence ${Date.now()}`;
  const changed = `${original} updated`;
  await notes.getByRole('textbox', { name: 'New private note', exact: true }).fill(original);
  await notes.getByRole('button', { name: 'Save note', exact: true }).click();
  const note = notes
    .locator('div')
    .filter({ has: page.getByText(original, { exact: true }) })
    .filter({ has: page.getByRole('button', { name: 'Edit note', exact: true }) })
    .last();
  await expect(notes.getByText(original, { exact: true })).toBeVisible();
  await note.getByRole('button', { name: 'Edit note', exact: true }).click();
  await notes.getByRole('textbox', { name: 'Edit private note', exact: true }).fill(changed);
  await notes.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(notes.getByText(changed, { exact: true })).toBeVisible();
  await page.reload();
  // The selected fact can change after reload; company notes retain their original target.
  await page
    .getByRole('button', { name: /^Chart / })
    .first()
    .click();
  await expect(notes.getByText(changed, { exact: true })).toBeVisible();
  const saved = notes
    .locator('div')
    .filter({ has: page.getByText(changed, { exact: true }) })
    .filter({ has: page.getByRole('button', { name: 'Delete note', exact: true }) })
    .last();
  await saved.getByRole('button', { name: 'Delete note', exact: true }).click();
  await expect(notes.getByText(changed, { exact: true })).toHaveCount(0);
  // Each research scope uses an explicit authorized anchor, with no company fallback.
  for (const target of [/^Reported fact:/, /^Filing:/, /^Company$/]) {
    await page.getByRole('combobox', { name: 'Note target', exact: true }).click();
    await page.getByRole('option', { name: target }).first().click();
    const scoped = `${original} ${target.source}`;
    await notes.getByRole('textbox', { name: 'New private note', exact: true }).fill(scoped);
    await notes.getByRole('button', { name: 'Save note', exact: true }).click();
    await expect(notes.getByText(scoped, { exact: true })).toBeVisible();
    const scopedNote = notes
      .locator('div')
      .filter({ has: page.getByText(scoped, { exact: true }) })
      .filter({ has: page.getByRole('button', { name: 'Delete note', exact: true }) })
      .last();
    await scopedNote.getByRole('button', { name: 'Delete note', exact: true }).click();
    await expect(notes.getByText(scoped, { exact: true })).toHaveCount(0);
  }
});

/** RFC 4180 parser for checking exact data cells, including quoted source labels. */
function csvRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [],
    cell = '',
    quoted = false;
  text = text.replace(/^\uFEFF/, '');
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === '"') {
      if (quoted && text[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else quoted = !quoted;
    } else if (char === ',' && !quoted) {
      row.push(cell);
      cell = '';
    } else if (char === '\n' && !quoted) {
      row.push(cell.replace(/\r$/, ''));
      rows.push(row);
      row = [];
      cell = '';
    } else cell += char;
  }
  if (cell || row.length) {
    row.push(cell);
    rows.push(row);
  }
  return rows;
}
