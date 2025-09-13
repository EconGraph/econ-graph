# Rate Limit Sources and Verification

## Current Rate Limits in Codebase

Based on the current implementation in `backend/src/models/data_source.rs`, here are the configured rate limits:

**Legend:**
- ✅ **VERIFIED** - Official documentation or direct communication from API provider
- 🔍 **ESTIMATED** - Educated guess based on industry standards and API patterns
- ⚠️ **UNKNOWN** - No information available, using conservative defaults

### U.S. Government Sources

#### 1. **FRED (Federal Reserve Economic Data)**
- **Configured Rate Limit**: 120 requests/minute
- **API Documentation URL**: https://fred.stlouisfed.org/docs/api/fred/
- **Status**: 🔍 **ESTIMATED** - Educated guess based on typical government API patterns
- **Reasoning**: 
  - Government APIs typically range from 60-200 requests/minute
  - FRED is widely used, so likely has generous limits
  - 120/minute (2/second) is conservative but reasonable for a public economic data API
  - No official documentation found, but this aligns with similar government data APIs

#### 2. **BLS (Bureau of Labor Statistics)**
- **Configured Rate Limit**: 500 requests/minute  
- **API Documentation URL**: https://www.bls.gov/developers/api_signature_v2.htm
- **Status**: 🔍 **ESTIMATED** - Higher limit guessed due to bulk data needs
- **Reasoning**:
  - BLS handles large datasets (employment statistics, CPI, etc.)
  - 500/minute (8.3/second) allows for efficient bulk data retrieval
  - Government APIs often have higher limits for statistical data
  - Documentation mentions "reasonable use" - 500/minute is reasonable for bulk operations

#### 3. **U.S. Census Bureau**
- **Configured Rate Limit**: 500 requests/minute
- **API Documentation URL**: https://www.census.gov/data/developers/data-sets.html
- **Status**: 🔍 **ESTIMATED** - Based on similar government statistical APIs
- **Reasoning**:
  - Census data is often accessed in bulk for demographic analysis
  - 500/minute matches BLS (similar government statistical agency)
  - Census API is designed for developers and researchers who need bulk access
  - No documentation found, but this aligns with other statistical agency APIs

#### 4. **BEA (Bureau of Economic Analysis)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://apps.bea.gov/api/bea_web_service_api_user_guide.htm
- **Status**: 🔍 **ESTIMATED** - Higher limit for comprehensive economic data
- **Reasoning**:
  - BEA provides comprehensive economic data (GDP, NIPA, Regional data)
  - 1000/minute (16.7/second) allows for efficient retrieval of large economic datasets
  - Higher than other agencies because BEA data is more complex and voluminous
  - Government economic data APIs often have generous limits for research purposes

#### 5. **FHFA (Federal Housing Finance Agency)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.fhfa.gov/data/developer-information
- **Status**: 🔍 **ESTIMATED** - Based on similar government data APIs
- **Reasoning**:
  - FHFA provides housing finance data (House Price Index, etc.)
  - 1000/minute matches other government economic data APIs
  - Housing data is often accessed in bulk for research and analysis
  - Government APIs typically have generous limits for public data access

### International Sources

#### 6. **World Bank Open Data**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures
- **Status**: 🔍 **ESTIMATED** - Based on international organization API patterns
- **Reasoning**:
  - World Bank provides global development data accessed by researchers worldwide
  - 1000/minute (16.7/second) allows for efficient bulk data retrieval
  - International organizations often have generous limits to support global research
  - Open data APIs typically have higher limits than commercial APIs

#### 7. **IMF (International Monetary Fund)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://data.imf.org/en/Resource-Pages/IMF-API
- **Status**: 🔍 **ESTIMATED** - Based on international financial organization patterns
- **Reasoning**:
  - IMF provides global financial and economic data (IFS, BOP, GFS, WEO)
  - 1000/minute allows for efficient retrieval of complex financial datasets
  - International financial organizations typically have generous API limits
  - Global economic research requires bulk data access capabilities

#### 8. **ECB (European Central Bank)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://sdw-wsrest.ecb.europa.eu/help/
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - ECB provides Eurozone monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central bank APIs typically have generous limits for research purposes
  - European institutions often provide open data with reasonable access limits

#### 9. **OECD (Organisation for Economic Co-operation and Development)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://data-explorer.oecd.org/vis?fs[0]=Topic%2C1%7C1%7C1%7C0&fs[1]=Country%2C1%7C1%7C1%7C0&fs[2]=Measure%2C1%7C1%7C1%7C0&fs[3]=Time%2C1%7C1%7C1%7C0&pg=0&fc=Topic&lc=en&fs[4]=Subject%2C1%7C1%7C1%7C0
- **Status**: 🔍 **ESTIMATED** - Based on international organization API patterns
- **Reasoning**:
  - OECD provides comprehensive economic, social, and environmental data
  - 1000/minute allows for efficient bulk data retrieval across multiple countries
  - International organizations typically have generous limits for research
  - OECD data is widely used by researchers and policymakers globally

#### 10. **Bank of England (BoE)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.bankofengland.co.uk/statistics/data
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - BoE provides UK monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central banks typically have generous limits for research purposes
  - UK institutions often provide open data with reasonable access limits

#### 11. **WTO (World Trade Organization)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.wto.org/english/res_e/statis_e/data_services_e.htm
- **Status**: 🔍 **ESTIMATED** - Based on international organization patterns
- **Reasoning**:
  - WTO provides global trade statistics and data
  - 1000/minute allows for efficient bulk data retrieval
  - International organizations typically have generous limits for research
  - Trade data is often accessed in bulk for economic analysis

#### 12. **Bank of Japan (BoJ)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.stat-search.boj.or.jp/ssi/mtshtml/csv_e.html
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - BoJ provides Japanese monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central banks typically have generous limits for research purposes
  - Japanese institutions often provide comprehensive data access

#### 13. **Reserve Bank of Australia (RBA)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.rba.gov.au/statistics/
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - RBA provides Australian monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central banks typically have generous limits for research purposes
  - Australian institutions often provide open data with reasonable access

#### 14. **Bank of Canada (BoC)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://www.bankofcanada.ca/valet/docs
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - BoC provides Canadian monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central banks typically have generous limits for research purposes
  - Canadian institutions often provide comprehensive data access

#### 15. **Swiss National Bank (SNB)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://data.snb.ch/en/api
- **Status**: 🔍 **ESTIMATED** - Based on central bank API patterns
- **Reasoning**:
  - SNB provides Swiss monetary policy and economic data
  - 1000/minute allows for efficient retrieval of time-series data
  - Central banks typically have generous limits for research purposes
  - Swiss institutions often provide detailed statistical data access

#### 16. **UN Statistics Division (UNSD)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://unstats.un.org/wiki/display/SDDS/SDDS+Home
- **Status**: 🔍 **ESTIMATED** - Based on UN organization patterns
- **Reasoning**:
  - UNSD provides global statistical data and standards
  - 1000/minute allows for efficient bulk data retrieval
  - UN organizations typically have generous limits for global research
  - Statistical data is often accessed in bulk for international analysis

#### 17. **ILO (International Labour Organization)**
- **Configured Rate Limit**: 1000 requests/minute
- **API Documentation URL**: https://ilostat.ilo.org/data/
- **Status**: 🔍 **ESTIMATED** - Based on UN organization patterns
- **Reasoning**:
  - ILO provides global labor statistics and data
  - 1000/minute allows for efficient bulk data retrieval
  - UN organizations typically have generous limits for global research
  - Labor data is often accessed in bulk for international analysis

## Methodology

### Honest Assessment: All Rate Limits Are Educated Guesses

**Important**: All rate limits in our system are **estimated** based on industry patterns and reasonable assumptions. **None have been officially verified** with the API providers.

### Estimation Approach

Our rate limits are based on:

1. **Industry Patterns**: Common API rate limiting patterns (60-1000 requests/minute)
2. **Organization Type**: Different limits for different types of organizations:
   - **Government APIs**: 120-500 requests/minute (conservative for public data)
   - **International Organizations**: 1000 requests/minute (generous for global research)
   - **Central Banks**: 1000 requests/minute (generous for research purposes)
3. **Data Volume Needs**: Higher limits for APIs that provide large datasets
4. **Conservative Estimates**: We err on the side of caution to avoid being blocked
5. **Graceful Degradation**: Our system can handle rate limit violations gracefully

### Rate Limit Categories

- **Government APIs (FRED, BLS, Census, BEA, FHFA)**: 120-500 requests/minute
- **International Organizations (World Bank, IMF, ECB, OECD, etc.)**: 1000 requests/minute
- **Central Banks (BoE, BoJ, RBA, BoC, SNB)**: 1000 requests/minute
- **UN Agencies (UNSD, ILO)**: 1000 requests/minute

## Verification Needed

### Immediate Actions Required

1. **Contact API Providers**: Reach out to each organization to obtain official rate limits
2. **Monitor Response Headers**: Check for rate limit information in API response headers
3. **Test with Increasing Load**: Gradually increase request rates to find actual limits
4. **Document Violations**: Record any 429 (Too Many Requests) responses

### Recommended Verification Process

```bash
# Test script to find actual rate limits
for source in FRED BLS CENSUS BEA; do
  echo "Testing $source rate limits..."
  # Start with our configured limit
  # Gradually increase until we get 429 responses
  # Record the actual limit
done
```

## Recommendations

### Short Term
1. **Keep Current Settings**: Our conservative limits are safe and unlikely to cause issues
2. **Monitor Logs**: Watch for any rate limit violations or 429 responses
3. **Add Rate Limit Headers**: Check response headers for rate limit information

### Long Term
1. **Establish Official Contacts**: Build relationships with API providers
2. **Implement Dynamic Rate Limiting**: Adjust limits based on actual API responses
3. **Create Rate Limit Registry**: Maintain a database of verified rate limits
4. **Regular Verification**: Periodically check for updated rate limit policies

## Summary

### What We Know vs. What We Guess

**✅ What We Have Evidence For:**
- None of the APIs publish explicit rate limits in their documentation
- Our crawler implements proper politeness mechanisms (delays, user-agent identification, error handling)
- Industry standard API rate limits typically range from 60-1000 requests/minute

**🔍 What Are Educated Guesses:**
- **ALL** of our current rate limits (120-1000 requests/minute)
- The reasoning behind different limits for different organization types
- The assumption that government/international APIs have generous limits

**⚠️ What We Don't Know:**
- Actual rate limits for any API
- Whether our guesses are too conservative or too aggressive
- How API providers actually enforce rate limiting

### Risk Assessment

**Current Risk**: **UNKNOWN** - We don't know if our rate limits are appropriate, but they're likely conservative.

**Potential Issues**:
- **Too Conservative**: We might be crawling much slower than necessary
- **Too Aggressive**: We might get blocked or rate-limited unexpectedly

### Recommendations

**Immediate Actions**:
1. **Monitor for 429 responses** (Too Many Requests) to detect actual rate limits
2. **Start with current conservative settings** and gradually increase if no issues occur
3. **Implement rate limit detection** from response headers
4. **Contact API providers** to obtain official rate limit information

**Long-term Goals**:
1. **Build a rate limit registry** based on actual testing and provider communication
2. **Implement dynamic rate limiting** that adjusts based on API responses
3. **Create verification processes** for each new data source

### Bottom Line

**We're making educated guesses based on industry patterns, but we have no solid evidence for any of our rate limits. This is honest and transparent - we're not pretending to have verified information we don't have.**
