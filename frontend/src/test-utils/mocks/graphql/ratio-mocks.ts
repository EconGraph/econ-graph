// MSW handlers for ratio analysis GraphQL queries
// These handlers provide realistic mock data for ratio analysis components

import { graphql, HttpResponse } from 'msw';
import { loadGraphQLResponse } from '../graphql-response-loader';

export const ratioHandlers = [
  graphql.query('GetFinancialRatios', ({ variables }) => {
    const { statementId } = variables as { statementId: string };
    let scenario = 'success';
    if (statementId === 'empty-statement-id') scenario = 'empty';
    if (statementId === 'partial-statement-id') scenario = 'partial';
    if (statementId === 'error-statement-id') scenario = 'error';

    const response = loadGraphQLResponse('get_financial_ratios', scenario);
    return HttpResponse.json(response, {
      status: (response as any).errors ? 400 : 200,
    });
  }),

  graphql.query('GetRatioBenchmarks', ({ variables }) => {
    const { ratioName } = variables as { ratioName: string };
    const scenario = ratioName === 'unknownRatio' ? 'not_found' : 'success';
    const response = loadGraphQLResponse('get_ratio_benchmarks', scenario);
    return HttpResponse.json(response, {
      status: (response as any).errors ? 400 : 200,
    });
  }),

  graphql.query('GetRatioExplanation', ({ variables }) => {
    const { ratioName } = variables as { ratioName: string };
    const scenario = ratioName === 'unknownRatio' ? 'not_found' : 'success';
    const response = loadGraphQLResponse('get_ratio_explanation', scenario);
    return HttpResponse.json(response, {
      status: (response as any).errors ? 400 : 200,
    });
  }),
];
