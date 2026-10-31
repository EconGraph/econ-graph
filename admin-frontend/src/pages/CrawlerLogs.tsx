import { useState } from "react";
import {
  Alert,
  Button,
  CircularProgress,
  FormControlLabel,
  MenuItem,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import { useCrawlerJobs } from "../hooks/useCrawlerAdmin";

export default function CrawlerLogs() {
  const [source, setSource] = useState("");
  const [status, setStatus] = useState("");
  const [offset, setOffset] = useState(0);
  const [polling, setPolling] = useState(true);
  const jobs = useCrawlerJobs(source, status, offset, polling);
  return (
    <Stack spacing={2}>
      <Typography variant="h5" component="h2">
        Crawler execution history
      </Typography>
      <Typography>
        Durable queue records show each job's latest execution state and
        worker-recorded errors. Polling runs every 10 seconds. These records are
        not a stream of process output or every retry attempt; detailed process
        logs remain in the deployment's logging system.
      </Typography>
      <Stack direction="row" spacing={2}>
        <TextField
          label="Source filter"
          helperText="Exact queue label, e.g. FRED"
          value={source}
          onChange={(event) => {
            setSource(event.target.value);
            setOffset(0);
          }}
        />
        <TextField
          select
          label="Status filter"
          value={status}
          onChange={(event) => {
            setStatus(event.target.value);
            setOffset(0);
          }}
          sx={{ minWidth: 150 }}
        >
          {[
            "",
            "pending",
            "processing",
            "retrying",
            "completed",
            "failed",
            "cancelled",
          ].map((value) => (
            <MenuItem key={value} value={value}>
              {value || "All statuses"}
            </MenuItem>
          ))}
        </TextField>
        <FormControlLabel
          control={
            <Switch
              checked={polling}
              onChange={(event) => setPolling(event.target.checked)}
            />
          }
          label="Poll history"
        />
        <Button onClick={() => jobs.refetch()}>Refresh</Button>
      </Stack>
      {jobs.isLoading && (
        <CircularProgress aria-label="Loading execution history" />
      )}
      {jobs.error && <Alert severity="error">{jobs.error.message}</Alert>}
      <Table aria-label="Crawler execution history">
        <TableHead>
          <TableRow>
            {["Updated", "Source / job", "State", "Retries", "Last error"].map(
              (label) => (
                <TableCell key={label}>{label}</TableCell>
              ),
            )}
          </TableRow>
        </TableHead>
        <TableBody>
          {((!jobs.error && jobs.data?.crawlerJobs) || []).map((job) => (
            <TableRow key={job.id}>
              <TableCell>{job.updatedAt}</TableCell>
              <TableCell>
                {job.source}: {job.seriesId}
                <br />
                {job.kind}
              </TableCell>
              <TableCell>{job.status}</TableCell>
              <TableCell>
                {job.retryCount} / {job.maxRetries}
              </TableCell>
              <TableCell>{job.errorMessage || "None recorded"}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {!jobs.isLoading && !jobs.error && !jobs.data?.crawlerJobs?.length && (
        <Typography>No matching queue records.</Typography>
      )}
      <Stack direction="row" spacing={2}>
        <Button
          disabled={offset === 0 || jobs.isFetching}
          onClick={() => setOffset(Math.max(0, offset - 50))}
        >
          Previous
        </Button>
        <Typography>
          Rows {offset + 1}–{offset + (jobs.data?.crawlerJobs?.length || 0)}
        </Typography>
        <Button
          disabled={
            jobs.isFetching ||
            (jobs.data?.crawlerJobs?.length || 0) < 50 ||
            offset >= 100_000
          }
          onClick={() => setOffset(offset + 50)}
        >
          Next
        </Button>
      </Stack>
    </Stack>
  );
}
