/**
 * Company Search Component
 *
 * Features:
 * - Fulltext search for companies using PostgreSQL indices
 * - Search by company name, ticker, or CIK
 * - Fuzzy search with spelling error tolerance
 * - Real-time search results
 * - Company selection for crawling
 */

import React, { useState, useCallback } from "react";
import {
  Box,
  TextField,
  Chip,
  Typography,
  Card,
  CardContent,
  CircularProgress,
  Alert,
  Button,
  IconButton,
  Tooltip,
  Paper,
  List,
  ListItem,
  ListItemText,
  ListItemSecondaryAction,
  Divider,
} from "@mui/material";
import {
  Search,
  Business,
  TrendingUp,
  Language,
  Info,
  PlayArrow,
} from "@mui/icons-material";
import { useCompanySearch } from "../../hooks/useCompanySearch";

interface Company {
  id: string;
  cik: string;
  ticker?: string;
  name: string;
  legal_name?: string;
  industry?: string;
  sector?: string;
  website?: string;
  phone?: string;
  state_of_incorporation?: string;
  is_active: boolean;
}

interface CompanySearchProps {
  onCompanySelect?: (company: Company) => void;
  onCrawlStart?: (company: Company) => void;
  placeholder?: string;
  showCrawlButton?: boolean;
  maxResults?: number;
}

export const CompanySearch: React.FC<CompanySearchProps> = ({
  onCompanySelect,
  onCrawlStart,
  placeholder = "Search companies by name, ticker, or CIK...",
  showCrawlButton = true,
  maxResults = 50,
}) => {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCompany, setSelectedCompany] = useState<Company | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  const { searchCompanies, companies, loading, error, totalCount } =
    useCompanySearch();

  // Debounced search function
  const debouncedSearch = useCallback(
    async (query: string) => {
      if (query.length < 2) return;

      setIsSearching(true);
      try {
        await searchCompanies({
          query,
          limit: maxResults,
          include_inactive: false,
        });
      } catch (err) {
        console.error("Search error:", err);
      } finally {
        setIsSearching(false);
      }
    },
    [searchCompanies, maxResults],
  );

  // Handle search input change
  const handleSearchChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const query = event.target.value;
      setSearchQuery(query);

      if (query.length >= 2) {
        debouncedSearch(query);
      }
    },
    [debouncedSearch],
  );

  // Handle company selection
  const handleCompanySelect = useCallback(
    (company: Company) => {
      setSelectedCompany(company);
      onCompanySelect?.(company);
    },
    [onCompanySelect],
  );

  // Handle crawl company
  const handleCrawlCompany = useCallback(
    (company: Company) => {
      onCrawlStart?.(company);
    },
    [onCrawlStart],
  );

  // Format company display name
  const formatCompanyName = useCallback((company: Company) => {
    const parts = [company.name];
    if (company.ticker) {
      parts.push(`(${company.ticker})`);
    }
    if (company.cik) {
      parts.push(`CIK: ${company.cik}`);
    }
    return parts.join(" ");
  }, []);

  // Get company industry/sector info
  const getCompanyInfo = useCallback((company: Company) => {
    const info = [];
    if (company.industry) info.push(company.industry);
    if (company.sector) info.push(company.sector);
    return info.join(" • ");
  }, []);

  return (
    <Box sx={{ width: "100%" }}>
      {/* Search Input */}
      <TextField
        fullWidth
        label="Search companies"
        placeholder={placeholder}
        value={searchQuery}
        onChange={handleSearchChange}
        InputProps={{
          startAdornment: <Search sx={{ color: "text.secondary", mr: 1 }} />,
          endAdornment: (loading || isSearching) && (
            <CircularProgress size={20} />
          ),
        }}
        sx={{ mb: 2 }}
        aria-describedby="search-help"
      />
      
      {/* Help text */}
      <Typography 
        id="search-help" 
        variant="caption" 
        color="text.secondary" 
        sx={{ display: "block", mb: 1 }}
      >
        Type at least 2 characters to search for companies
      </Typography>

      {/* Search Results */}
      {searchQuery.length >= 2 && (
        <Paper 
          elevation={2} 
          sx={{ maxHeight: 400, overflow: "auto" }}
          role="region"
          aria-label="Search results"
          aria-live="polite"
        >
          {loading || isSearching ? (
            <Box sx={{ p: 2, textAlign: "center" }}>
              <CircularProgress size={24} />
              <Typography variant="body2" sx={{ mt: 1 }}>
                Searching companies...
              </Typography>
            </Box>
          ) : error ? (
            <Alert severity="error" sx={{ m: 2 }}>
              Error searching companies: {error.message}
            </Alert>
          ) : companies.length === 0 ? (
            <Box sx={{ p: 2, textAlign: "center" }}>
              <Typography variant="body2" color="text.secondary">
                No companies found for "{searchQuery}"
              </Typography>
            </Box>
          ) : (
            <List dense>
              {companies.map((company, index) => (
                <React.Fragment key={company.id}>
                  <ListItem
                    button
                    onClick={() => handleCompanySelect(company)}
                    sx={{
                      "&:hover": {
                        backgroundColor: "action.hover",
                      },
                    }}
                    aria-label={`Select ${company.name}`}
                    role="button"
                    tabIndex={0}
                  >
                    <ListItemText
                      primary={
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                        >
                          <Business fontSize="small" color="primary" />
                          <Typography variant="subtitle2" noWrap>
                            {formatCompanyName(company)}
                          </Typography>
                          {!company.is_active && (
                            <Chip
                              label="Inactive"
                              size="small"
                              color="warning"
                              variant="outlined"
                            />
                          )}
                        </Box>
                      }
                      secondary={
                        <Box>
                          {getCompanyInfo(company) && (
                            <Typography
                              variant="body2"
                              color="text.secondary"
                              noWrap
                            >
                              {getCompanyInfo(company)}
                            </Typography>
                          )}
                          {company.website && (
                            <Box
                              sx={{
                                display: "flex",
                                alignItems: "center",
                                gap: 0.5,
                                mt: 0.5,
                              }}
                            >
                              <Language fontSize="small" color="action" />
                              <Typography
                                variant="caption"
                                color="text.secondary"
                              >
                                {company.website}
                              </Typography>
                            </Box>
                          )}
                        </Box>
                      }
                    />
                    <ListItemSecondaryAction>
                      <Box sx={{ display: "flex", gap: 1 }}>
                        <Tooltip title="View Details">
                          <IconButton
                            size="small"
                            onClick={() => handleCompanySelect(company)}
                            aria-label={`View details for ${company.name}`}
                          >
                            <Info fontSize="small" />
                          </IconButton>
                        </Tooltip>
                        {showCrawlButton && (
                          <Tooltip title="Crawl Company">
                            <IconButton
                              size="small"
                              onClick={() => handleCrawlCompany(company)}
                              color="primary"
                              aria-label={`Crawl filings for ${company.name}`}
                            >
                              <PlayArrow fontSize="small" />
                            </IconButton>
                          </Tooltip>
                        )}
                      </Box>
                    </ListItemSecondaryAction>
                  </ListItem>
                  {index < companies.length - 1 && <Divider />}
                </React.Fragment>
              ))}
            </List>
          )}
        </Paper>
      )}

      {/* Selected Company */}
      {selectedCompany && (
        <Card sx={{ mt: 2 }} role="region" aria-label="Selected company">
          <CardContent>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 1 }}>
              <Business color="primary" />
              <Typography variant="h6">{selectedCompany.name}</Typography>
              {selectedCompany.ticker && (
                <Chip
                  label={selectedCompany.ticker}
                  size="small"
                  color="primary"
                  variant="outlined"
                />
              )}
            </Box>

            <Box sx={{ display: "flex", flexWrap: "wrap", gap: 1, mb: 2 }}>
              <Chip
                icon={<TrendingUp />}
                label={`CIK: ${selectedCompany.cik}`}
                size="small"
                variant="outlined"
              />
              {selectedCompany.industry && (
                <Chip
                  label={selectedCompany.industry}
                  size="small"
                  variant="outlined"
                />
              )}
              {selectedCompany.sector && (
                <Chip
                  label={selectedCompany.sector}
                  size="small"
                  variant="outlined"
                />
              )}
            </Box>

            {showCrawlButton && (
              <Button
                variant="contained"
                startIcon={<PlayArrow />}
                onClick={() => handleCrawlCompany(selectedCompany)}
                fullWidth
                aria-label={`Crawl filings for ${selectedCompany.name}`}
              >
                Crawl Company Filings
              </Button>
            )}
          </CardContent>
        </Card>
      )}

      {/* Search Stats */}
      {companies.length > 0 && (
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ mt: 1, display: "block" }}
        >
          Found {companies.length} of {totalCount} companies
        </Typography>
      )}
    </Box>
  );
};

export default CompanySearch;
