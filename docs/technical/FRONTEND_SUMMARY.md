# React Frontend - EconGraph

## Overview

The EconGraph frontend is a modern React application built with TypeScript and Material UI that provides an intuitive interface for exploring economic time series data. It significantly improves upon FRED's user experience with modern design patterns and responsive interactions.

## Frontend Features

### Interactive Charts and Revision Information

The series detail page plots the latest stored revision for each observation date. Hover tooltips show the observation date and formatted value with units, plus a revision date when it differs from the observation date. The recent-observations table labels each value **Original** or **Revised**, making revision provenance visible alongside the data.

- **Date range controls:** Date pickers narrow the visible time period without leaving the chart.
- **Transformations:** Year-over-year, quarter-over-quarter, and month-over-month percentage changes are offered where appropriate for the series frequency. The selector also offers change since the first available value and log difference.
- **Consistent labels:** The chart title, axis, tooltips, and recent-observations table reflect the transformation and its units.

Selecting a transformation requests new values from the backend without reloading the page. While that request loads, the previous result stays visible with its own transformation label, so a new selection does not mislabel the values on screen.

### Search and Filtering

Submitting the header search opens the series explorer at `/explore?q=…`. In the explorer, searches run after a 250 ms pause in typing once the trimmed query contains at least two characters. Source and frequency filters narrow the results; selecting a source also allows browsing its series without entering a search term. The query and filter choices are stored in the URL so the view can be bookmarked or shared.

### Navigation and Mobile Layout

React Router connects the dashboard, series explorer, series detail pages, data sources, and global analysis. Below 600px, navigation uses a modal drawer that starts closed and opens from the header menu button. At wider widths, the sidebar starts open beside the page and can be hidden to give the content more space. Charts resize to their container, and chart controls wrap on narrow screens.

Light and dark themes let readers choose the display they prefer, with the choice saved in the browser. Together, the search, chart controls, and responsive navigation support exploration from a single interface, while revision labels and source metadata keep the origin of the numbers in view.

## Project Structure

This selected map shows the entry point and the main series-exploration components:

```text
frontend/
├── index.html                 # Vite HTML entry point
├── src/
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Header.tsx      # App header with search
│   │   │   └── Sidebar.tsx     # Responsive navigation
│   │   └── charts/
│   │       └── SeriesChart.tsx # Series plot and chart controls
│   ├── contexts/
│   │   └── ThemeContext.tsx    # Light/dark themes and preference
│   ├── pages/
│   │   ├── Dashboard.tsx       # Overview dashboard
│   │   ├── SeriesExplorer.tsx  # Browse and search series
│   │   ├── SeriesDetail.tsx    # Detailed series view
│   │   ├── DataSources.tsx     # Data source information
│   │   └── About.tsx           # About page
│   ├── hooks/
│   │   └── useSeriesData.ts    # Series, search, and source hooks
│   ├── utils/
│   │   └── graphql.ts         # GraphQL operations and transport
│   ├── App.tsx                # Routes and shared page layout
│   ├── index.tsx              # React root and query client
│   └── index.css              # Global styles
├── package.json               # Dependencies and scripts
├── tsconfig.json              # TypeScript configuration
└── vite.config.ts             # Development proxy and build configuration
```

## Technology Stack

The [package manifest](../../frontend/package.json) records dependency versions and development commands.

### Core Framework and UI

- **React 19 and TypeScript:** Components with typed props, hooks, and strict type checking.
- **React Router 7:** Client-side routes and URL search parameters.
- **Material UI 9:** Themed components, layouts, and icons.
- **Emotion:** CSS-in-JS styling used by the UI components.

### Charts and Visualization

- **Chart.js 4:** Canvas-based time-series charts.
- **react-chartjs-2:** React integration for Chart.js.
- **date-fns and chartjs-adapter-date-fns:** Date formatting and time-axis support.
- **Material UI date pickers:** Custom date-range controls.

### State Management and Data Fetching

- **React Query:** Caching, request state, and background refetches.
- **GraphQL:** Data fetching with explicit field selection.
- **Custom hooks:** Reusable series, search, and source queries.

### Developer Tools

- **Vite:** Development server and production bundling.
- **React Query Devtools:** Query inspection in development.
- **TypeScript strict mode and ESLint:** Type checking and code consistency.

## GraphQL Integration and Caching

Pages use the hooks in [useSeriesData.ts](../../frontend/src/hooks/useSeriesData.ts) to coordinate fetching. The hooks call operations defined in [graphql.ts](../../frontend/src/utils/graphql.ts), and React Query tracks their cached results, loading state, and errors. Pages use that state to show loading placeholders and error messages with retry controls.

For example, `useSeriesData` caches observations by series ID, transformation, start date, and end date. A change to any of those inputs selects a different cache entry. The hook treats observations as fresh for five minutes and retains the previous result while another transformation loads. The series detail page labels the transformation carried by the displayed result rather than assuming the newest selection has already loaded.

The observation fetcher follows GraphQL cursor pages until it has read the full result, requesting up to 10,000 points per page. These are successive requests, with a failed or stalled page treated as an error so a partial series is not displayed as complete. Transformations are computed by the backend; the chart then filters the returned points to the visible date range.

The transport routes economic-data queries to `VITE_DATA_GRAPHQL_URL`, falling back to `VITE_GRAPHQL_URL` and then `/graphql`. App operations, such as annotation writes, use `VITE_GRAPHQL_URL` with the user's access token. This lets a deployment share a data plane while keeping user and application state in its app plane.

### Main Series Queries

| Query constant | Role |
| --- | --- |
| `GET_SERIES_LIST` | Browse series with filters and pagination. |
| `GET_SERIES_DETAIL` | Retrieve series metadata, including source, frequency, units, and date coverage. |
| `GET_SERIES_DATA` | Retrieve observations with a backend transformation and revision filter. |
| `SEARCH_SERIES` | Search series with source and frequency filters and relevance information. |
| `GET_DATA_SOURCES` | Retrieve provider descriptions, series counts, and API requirements. |

## Design System

[ThemeContext.tsx](../../frontend/src/contexts/ThemeContext.tsx) defines the light and dark themes. The light theme uses blue `#1976d2` as its primary color and red `#dc004e` as its secondary color. The dark theme uses lighter blue `#90caf9` and pink `#f48fb1`. Both use a Roboto-first font stack and an 8px spacing unit.

The themes keep Material UI's default breakpoints: `sm` at 600px, `md` at 900px, `lg` at 1200px, and `xl` at 1536px. Individual components use these breakpoints to adapt controls and layout; the sidebar switches between its mobile drawer and desktop panel at `sm`.

Navigation has a labeled landmark and marks the active route with `aria-current`. The header search has an accessible label, and the recent-observations table provides a text view of the latest values alongside the canvas chart. Global CSS includes contrast and reduced-motion rules for page styling. The [mobile sidebar regression test](../../frontend/src/__tests__/app-sidebar-mobile.test.tsx) checks that the closed mobile drawer leaves the page accessible to assistive technology.

## Performance

React Query reuses fresh cached responses, and the chart memoizes filtered points, datasets, and options. The [Vite configuration](../../frontend/vite.config.ts) groups libraries such as React, Material UI, and Chart.js into separate chunks for browser caching.

Core pages are imported eagerly in [App.tsx](../../frontend/src/App.tsx). Conditional routes use lazy imports controlled by [build flags](../../frontend/docs/build-flags.md); lazy loading is scoped to those routes.

## Testing

Vitest and React Testing Library cover component behavior and query hooks. Mock Service Worker supplies API responses for tests, and Playwright covers browser workflows, including mobile configurations. Storybook supports component previews and interaction testing; see the [Storybook testing guide](../../frontend/docs/storybook-testing.md).

For setup and development commands, see the [quick start](../../README.md#-quick-start).
