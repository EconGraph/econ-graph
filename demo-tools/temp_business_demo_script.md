# 🎯 REAL UI BUSINESS DEMO SCRIPT (3 minutes)

## 📱 What You'll Demonstrate

- The real React application running at `http://localhost:3000`
- Economic data discovery, interactive charts, and CSV export
- Business value for economists and analysts in a professional Material-UI interface

## 🎬 Preflight and Recording Setup

Complete setup and rehearsal before starting the narration clock.

1. Install the [development prerequisites](../README.md#prerequisites), then follow the backend's [local database and server setup](../backend/README.md#development-workflow). Configure both `APP_DATABASE_URL` and `DATA_DATABASE_URL` and apply the data plane migrations. Start the server from `backend/` with:

   ```bash
   cargo run -p econ-graph-backend --bin econ-graph-backend
   ```

   The backend applies app plane migrations at startup and listens on port 9876 by default. A fresh database has no series observations; use the [crawler deployment guide](../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md) to populate the data database before recording.

2. In a second terminal, starting from the repository root, install frontend dependencies and align Vite's proxy with the backend:

   ```bash
   cd frontend
   npm ci
   BACKEND_URL=http://localhost:9876 npm start
   ```

   If the backend uses a different port, set `BACKEND_URL` to that address instead. Open `http://localhost:3000`.

3. Confirm that the dashboard's configured indicators show values and observation dates rather than “No data yet”; the chosen explorer series may differ from those indicators. Choose a populated source and series, then rehearse the entire route below. Write down the exact source, search query, series title, frequency, date range, and transformation you will use. A query such as “GDP” or “inflation” is useful only if it returns the chosen series in this database. Confirm that the chart has enough observations for the chosen transformation, that the source and frequency filters retain the result, and that **Download CSV** produces the displayed observations. If using fixtures, identify them as fixture data in the narration rather than implying a fresh upstream fetch.
4. Prepare the browser's Network panel so you can show a GraphQL request without exposing tokens or private information. Rehearse the narration and clicks together to fit each timed segment, including loading time.
5. In QuickTime, choose **New Screen Recording**, or use another screen recorder. Capture only the browser area. Disable click indicators and notifications, and begin on the loaded dashboard.

---

## 🎤 Demo Script and Actions

### [0–15 seconds] Opening and Value Proposition

**SAY:** “This is EconGraph, a working economic data visualization application. You're seeing the actual React application running locally, with real interactions throughout this demonstration.”

**DO:**

- Show the loaded dashboard and its latest-value cards.
- Point out the Material-UI interface and navigation menu.

### [15–45 seconds] Core Business Value

**SAY:** “For economists and analysts, EconGraph brings discovery and analysis into one workflow. Start with an economic data source, find a relevant indicator, and open its history without moving between separate tools.”

**DO:**

- Click **Data Sources** and show the chosen source's information.
- Click that source's **Browse Series** link to open the Series Explorer with the source selected.
- Type the rehearsed query into the explorer's search field, wait for matching results, and point out the chosen series.

### [45–75 seconds] Interactive Charts

**SAY:** “The series chart lets you inspect individual observations and focus on a period of interest. Transformations make changes over time easier to examine and compare with the original levels.”

**DO:**

- Click the chosen result to open its series detail page.
- Hover over a data point to show its tooltip.
- Set **Start Date** and **End Date** to the rehearsed range.
- Choose the rehearsed option from **Transformation**, then return to **None** to show the original levels. Options depend on frequency: demonstrate only those offered for this series, such as **Year-over-Year** or **Month-over-Month**.

### [75–105 seconds] Technical Capabilities

**SAY:** “Behind this interface are a React frontend, a Rust backend with a GraphQL API, and PostgreSQL storage. The browser requests economic series and observations through that API, connecting discovery and visualization in a full-stack application.”

**DO:**

- Briefly open the prepared Network panel and show a GraphQL request from the preceding interaction.
- Close developer tools, then resize the browser to demonstrate the responsive layout.
- Restore the recording width before continuing.

### [105–135 seconds] Search and Data Management

**SAY:** “Search helps analysts find indicators by topic, while source and frequency filters narrow the results. That makes it easier to locate the right series and see where it comes from before using it in analysis.”

**DO:**

- In the header search field, type the rehearsed query and press Enter to open the explorer results.
- Select the chosen **Source** and **Frequency**, waiting for results to settle after each change.
- Show the matching series cards, then click the chosen result to reopen its detail page.

### [135–165 seconds] Business Impact

**SAY:** “For research teams, financial institutions, agencies, and academic researchers, the value is a shorter path from finding an indicator to using it. Download the observations as CSV for spreadsheet work or to share with colleagues, carrying the selected period and transformation into the export.”

**DO:**

- Set the rehearsed date range and transformation on the series detail page.
- Click **Download CSV** once observations finish loading.
- Show the downloaded file in the browser's download list and explain how it supports further analysis.

### [165–180 seconds] Closing

**SAY:** “EconGraph brings economic data discovery, interactive analysis, and export into one professional interface. Find an indicator, explore its history, and take the observations into your next research task.”

**DO:**

- Return to the dashboard.
- Hold on the EconGraph title and the application for the final seconds.

---

## 💡 Business Points to Emphasize

- **One workflow:** Move from source information to matching series, charts, and exported observations.
- **Less preparation between tools:** Inspect dates and transformations before taking the data into a spreadsheet.
- **Professional presentation:** Connect each visible interaction to an economist's or analyst's research task.

## 🎬 Recording Tips

1. **Move deliberately:** Keep pointer movements smooth and pause after each click so viewers can follow the interaction.
2. **Let results load:** Narrate what is visible; leave time for search results and transformed observations to finish loading.
3. **Keep controls readable:** Use a browser width and zoom level that show chart labels, filters, and source details clearly.
4. **Keep the business focus:** Explain how each action helps locate, inspect, or reuse economic data.
5. **Use the rehearsal:** If a query or date range produces no usable observations, choose another before recording.

## 🎯 Success Criteria

- The recording shows the real application and populated data, with fixtures identified if used.
- Every narrated feature has a visible interaction, from source browsing through CSV download.
- Navigation and narration fit the continuous three-minute schedule.
- The presentation stays readable, confident, and focused on business value.
