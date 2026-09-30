// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import React from 'react';
import { Typography } from '@mui/material';

/**
 * Behind the `build_canary` flag, which is off in release builds. The marker text below is
 * what scripts/check-flag-bundle.mjs looks for: it must be absent from the release bundle
 * and present in the dev bundle. Keep the string unique to this file.
 * @returns The canary page.
 */
export default function FlagCanary(): React.JSX.Element {
  return (
    <Typography variant='body1'>Build flag canary is on: econgraph-flag-canary-5b1e9d</Typography>
  );
}
