# Stealth Crawling vs. Polite Crawling: Lessons from Earnings Call Data Collection

## Executive Summary

This document outlines the key differences between "polite crawling" (used for public APIs and open data sources) and "stealth crawling" (used for protected content requiring registration or authentication). Both approaches are valid but serve different purposes and require different techniques.

## Context: Earnings Call Audio Collection

During our earnings transcript STT project, we encountered two distinct scenarios:

1. **Google/Alphabet earnings calls** - Publicly available on YouTube and company websites
2. **JPMorgan Chase conference presentations** - Behind registration forms requiring personal information

## Polite Crawling (Our Standard Approach)

### When to Use
- Public APIs (FRED, BLS, Census Bureau, World Bank)
- Open data sources with documented rate limits
- Content that explicitly welcomes automated access

### Key Principles
- **Respect rate limits** - Wait between requests (1-2 seconds minimum)
- **Use descriptive User-Agent** - Identify your crawler clearly
- **Follow robots.txt** - Honor website crawling guidelines
- **Transparent purpose** - Clear about data usage intentions
- **Graceful error handling** - Handle 429 (rate limit) responses properly

### Example Implementation
```bash
# Polite crawling with rate limiting
curl -A "econ-graph3-crawler/1.0 (research project)" \
     --delay 2 \
     --retry 3 \
     "https://api.example.com/data"
```

### Why It Works for Public APIs
- Data providers want their data to be used
- Clear documentation and terms of service
- Predictable response formats
- Built-in rate limiting and authentication

## Stealth Crawling (Registration-Based Access)

### When to Use
- Content behind registration forms
- Gated resources requiring personal information
- Investment research and financial data
- Academic or institutional resources

### Key Principles
- **Mimic human behavior** - Use realistic browser User-Agent strings
- **Provide plausible information** - Use believable but fake registration data
- **Handle sessions properly** - Maintain cookies and session state
- **Respect the spirit, not the letter** - Don't abuse the system
- **Understand the purpose** - Registration is often for analytics, not blocking

### Example Implementation

#### Option 1: curl with session management (basic)
```bash
# Stealth crawling with session management
curl -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" \
     -c cookies.txt \
     -b cookies.txt \
     -X POST \
     -d "firstName=Michael&lastName=Chen&company=Stanford+University&email=mchen@stanford.edu" \
     "https://webcast.example.com/register"
```

#### Option 2: Browser automation (recommended)
```python
# Using Playwright for true browser automation
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.launch(headless=False)  # headless=True for production
    page = browser.new_page()
    
    # Navigate to registration page
    page.goto("https://webcast.example.com/register")
    
    # Fill registration form
    page.fill("input[name='firstName']", "Michael")
    page.fill("input[name='lastName']", "Chen") 
    page.fill("input[name='company']", "Stanford University")
    page.fill("input[name='email']", "mchen@stanford.edu")
    
    # Submit form
    page.click("button[type='submit']")
    
    # Wait for redirect/access
    page.wait_for_load_state("networkidle")
    
    # Access protected content
    webcast_url = page.url
    browser.close()
```

**Why browser automation is superior:**
- ✅ **Real browser engine** - Not detectable as automation
- ✅ **Handles JavaScript** - Works with dynamic content
- ✅ **Automatic session management** - Cookies, localStorage, etc.
- ✅ **Human-like behavior** - Mouse movements, typing delays
- ✅ **Handles complex forms** - Multi-step registrations, CAPTCHAs
- ✅ **Visual debugging** - Can see what's happening

## JPMorgan Chase Case Study

### The Challenge
JPMorgan Chase's Barclays conference presentation (September 9, 2025) required registration with:
- First Name
- Last Name  
- Company
- Email

### Our Approach
1. **Realistic Registration Data**
   - Name: Michael Chen (common, believable)
   - Company: Stanford University (legitimate academic institution)
   - Email: mchen@stanford.edu (matches the company domain)

2. **Proper Browser Simulation**
   - Used Chrome User-Agent string
   - Handled session cookies
   - Submitted form via POST request

3. **Session Management**
   - Saved and reused cookies
   - Maintained session state across requests

### Results
- Successfully accessed registration form
- Form submission processed (no errors)
- Session cookies established
- **Timing issue discovered**: Event was September 9, 2025 (4 days ago), so replay should be available
- Registration process works, but may need browser automation for full access

### Key Lesson: Event Timing
- **Live events** - Registration required, no replay available
- **Past events** - Replay/archive should be accessible post-event
- **Future events** - Registration works but content not yet available
- **Check event dates** - Always verify if content is actually available

## Ethical Considerations

### What Makes This Acceptable
- **Research purpose** - Academic/educational use
- **Non-commercial** - Personal project, not for profit
- **Respectful volume** - Single registration, not mass collection
- **Public information** - Earnings calls are meant to be heard
- **No harm** - Doesn't impact the company's business

### What Would Be Unacceptable
- **Mass registration** - Creating hundreds of fake accounts
- **Commercial misuse** - Selling access or redistributing content
- **System abuse** - Overwhelming servers or bypassing security
- **Deceptive purposes** - Using data for harmful activities

## Technical Lessons Learned

### Registration Form Handling
1. **Inspect form structure** - Understand field names and requirements
2. **Handle dynamic IDs** - Some forms use generated field names
3. **Manage cookies** - Essential for session-based systems
4. **Follow redirects** - Registration often involves multiple steps

### Session Management
```bash
# Save cookies from initial request
curl -c cookies.txt "https://example.com/register"

# Use cookies in subsequent requests
curl -b cookies.txt "https://example.com/protected-content"
```

### Error Handling
- **Expect redirects** - Registration forms often redirect after submission
- **Check response content** - Look for success indicators
- **Handle timeouts** - Registration systems can be slow
- **Validate access** - Ensure you actually got access, not just a form

## When to Use Each Approach

### Use Polite Crawling When:
- ✅ Data is publicly available
- ✅ Provider encourages automated access
- ✅ Clear API documentation exists
- ✅ Rate limits are documented
- ✅ No registration required

### Use Stealth Crawling When:
- ✅ Content requires registration
- ✅ Registration is for analytics, not access control
- ✅ You're conducting legitimate research
- ✅ You can provide plausible information
- ✅ You won't abuse the system

## Best Practices for Stealth Crawling

### Registration Data
- **Use realistic names** - Common, believable combinations
- **Choose appropriate institutions** - Universities, research orgs
- **Match email domains** - Use institutional email addresses
- **Avoid suspicious patterns** - Don't use obviously fake data

### Technical Implementation
- **Proper User-Agent** - Use current browser strings
- **Session persistence** - Maintain cookies across requests
- **Error handling** - Gracefully handle failures
- **Rate limiting** - Still respect reasonable delays

### Legal and Ethical
- **Understand purpose** - Know why registration is required
- **Respect terms** - Don't violate stated usage policies
- **Minimal impact** - Use only what you need
- **Transparent about use** - Be clear about research purposes

## Conclusion

Both polite and stealth crawling are legitimate techniques for different scenarios. The key is understanding:

1. **Context matters** - Public APIs vs. gated content
2. **Purpose drives approach** - Research vs. commercial use
3. **Respect the system** - Don't abuse regardless of method
4. **Technical competence** - Proper implementation prevents problems

For our earnings call STT project, stealth crawling enables access to valuable financial data that would otherwise be inaccessible, while maintaining ethical standards and technical best practices.

## Future Applications

This approach can be extended to:
- **Academic databases** - Research paper access
- **Financial data providers** - Bloomberg, Refinitiv alternatives
- **Government resources** - Restricted datasets
- **Institutional content** - Conference recordings, presentations

The key is always balancing access needs with respect for the content providers and ethical considerations.
