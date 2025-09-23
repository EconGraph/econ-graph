import React from 'react';
import { render, RenderOptions } from '@testing-library/react';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CssBaseline, StyledEngineProvider } from '@mui/material';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { BrowserRouter } from 'react-router-dom';

// Create a consistent theme for testing
export const testTheme = createTheme({
  palette: {
    mode: 'light',
    primary: {
      main: '#1976d2',
    },
    secondary: {
      main: '#dc004e',
    },
  },
});

// Comprehensive Material-UI test wrapper with Router support
export const MaterialUITestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <BrowserRouter>
    <StyledEngineProvider injectFirst>
      <ThemeProvider theme={testTheme}>
        <LocalizationProvider dateAdapter={AdapterDateFns}>
          <CssBaseline />
          {children}
        </LocalizationProvider>
      </ThemeProvider>
    </StyledEngineProvider>
  </BrowserRouter>
);

// Custom render function that handles Material-UI portals properly
export const customRender = (ui: React.ReactElement, options: RenderOptions = {}) => {
  // Create a proper container instead of using document.body
  const container = document.createElement('div');
  container.setAttribute('data-testid', 'test-container');
  document.body.appendChild(container);

  // Create a portal container for Material-UI dialogs
  const portalContainer = document.createElement('div');
  portalContainer.setAttribute('data-testid', 'material-ui-portal-container');
  document.body.appendChild(portalContainer);

  const renderResult = render(ui, {
    wrapper: MaterialUITestWrapper,
    container: container, // Use proper container instead of document.body
    ...options,
  });

  return {
    ...renderResult,
    portalContainer,
    cleanup: () => {
      // Clean up portal container
      if (document.body.contains(portalContainer)) {
        document.body.removeChild(portalContainer);
      }
      // Clean up test container
      if (document.body.contains(container)) {
        document.body.removeChild(container);
      }
    },
  };
};

// Test cleanup utilities
export const setupTestEnvironment = () => {
  // Clean up any existing portal containers
  const existingContainers = document.querySelectorAll(
    '[data-testid="material-ui-portal-container"]'
  );
  existingContainers.forEach(container => container.remove());

  // Clean up any existing test containers
  const existingTestContainers = document.querySelectorAll('[data-testid="test-container"]');
  existingTestContainers.forEach(container => container.remove());
};

export const cleanupTestEnvironment = () => {
  // Clean up all portal containers
  const containers = document.querySelectorAll('[data-testid="material-ui-portal-container"]');
  containers.forEach(container => container.remove());
};

// Helper function to wait for Material-UI dialogs to render
export const waitForDialog = async (dialogTitle?: string) => {
  const { waitFor } = await import('@testing-library/react');

  await waitFor(
    () => {
      if (dialogTitle) {
        const dialog = document.querySelector(`[role="dialog"]`);
        expect(dialog).toBeInTheDocument();
        // Check if dialog title matches (flexible matching)
        const titleElement = document.querySelector(
          '[role="dialog"] h2, [role="dialog"] h1, [role="dialog"] [class*="DialogTitle"]'
        );
        if (titleElement) {
          expect(titleElement.textContent).toContain(dialogTitle);
        }
      } else {
        expect(document.querySelector('[role="dialog"]')).toBeInTheDocument();
      }
    },
    { timeout: 5000 }
  );
};

// Helper function to find form fields in dialogs
export const findFormFieldInDialog = (
  fieldType: 'input' | 'textarea' | 'select',
  fieldName: string
) => {
  const dialog = document.querySelector('[role="dialog"]');
  if (!dialog) return null;

  // Try different strategies to find the field
  // 1. By aria-label
  let field = dialog.querySelector(`${fieldType}[aria-label*="${fieldName}"]`);
  if (field) return field;

  // 2. By placeholder
  field = dialog.querySelector(`${fieldType}[placeholder*="${fieldName}"]`);
  if (field) return field;

  // 3. By associated label
  const labels = dialog.querySelectorAll('label');
  for (const label of labels) {
    if (label.textContent?.toLowerCase().includes(fieldName.toLowerCase())) {
      const labelFor = label.getAttribute('for');
      if (labelFor) {
        // Escape special characters in the ID for CSS selector
        const escapedId = labelFor.replace(/[!"#$%&'()*+,.\/:;<=>?@[\\\]^`{|}~]/g, '\\$&');
        field = dialog.querySelector(`#${escapedId}`);
        if (field) return field;
      }
    }
  }

  // 4. By data-testid
  field = dialog.querySelector(`${fieldType}[data-testid*="${fieldName.toLowerCase()}"]`);
  if (field) return field;

  return null;
};

// Re-export everything from React Testing Library
export * from '@testing-library/react';
export { customRender as render };
