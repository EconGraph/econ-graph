/**
 * Professional Collaboration-Focused Demo Video Creator
 * Showcases real-time collaboration features for EconGraph
 */

const { chromium } = require('playwright');
const path = require('path');

const DEMO_CONFIG = {
  baseUrl: 'http://localhost:3000',
  width: 1920,
  height: 1080,
  typingDelay: 100,
  actionDelay: 1500,
  videoPath: path.join(__dirname, 'demo-videos', 'collaboration-demo-with-narration.webm'),
  audioNarration: true
};

// Enhanced Collaboration-Focused Narration Script
const COLLABORATION_NARRATION_SCRIPT = [
  {
    timestamp: 0,
    text: "Welcome to EconGraph's Revolutionary Collaboration Platform - where economic analysis meets real-time teamwork. Today we'll explore how teams collaborate on economic research like never before.",
    duration: 8000
  },
  {
    timestamp: 8500,
    text: "Let's start with our Bloomberg Terminal-inspired collaboration workspace. Notice the real-time collaboration panel showing active team members working on this analysis.",
    duration: 7000
  },
  {
    timestamp: 16000,
    text: "Here we see live collaboration indicators - green dots show who's currently online, their roles, and their permission levels for this economic analysis.",
    duration: 6000
  },
  {
    timestamp: 23000,
    text: "Now let's demonstrate real-time chart annotations. I'll add a professional annotation marking the COVID-19 economic impact period with contextual analysis.",
    duration: 7000
  },
  {
    timestamp: 31000,
    text: "Watch as I create a vertical line annotation at March 2020. The system provides multiple annotation types: vertical lines, data points, range boxes, and trend lines.",
    duration: 7000
  },
  {
    timestamp: 39000,
    text: "I'm adding a detailed description explaining the economic significance of this date. Notice the professional color coding and tagging system for organizing annotations.",
    duration: 7000
  },
  {
    timestamp: 47000,
    text: "The annotation appears instantly on the chart with professional styling. Team members can see this annotation in real-time, enabling collaborative economic analysis.",
    duration: 7000
  },
  {
    timestamp: 55000,
    text: "Now let's explore the comment threading system. Team members can discuss annotations directly, creating threaded conversations about specific economic events.",
    duration: 7000
  },
  {
    timestamp: 63000,
    text: "I'm adding a comment to this annotation discussing the Federal Reserve's policy response. Notice the real-time timestamp and author attribution.",
    duration: 7000
  },
  {
    timestamp: 71000,
    text: "The collaboration panel shows all team annotations with filtering options: view all annotations, only mine, or just the pinned important ones.",
    duration: 7000
  },
  {
    timestamp: 79000,
    text: "Let's demonstrate the annotation management features. Users can toggle visibility, pin important annotations, and organize by tags for complex analysis workflows.",
    duration: 8000
  },
  {
    timestamp: 88000,
    text: "The permission system ensures secure collaboration. Team leads can control who can view, comment, edit, or administer collaborative charts and annotations.",
    duration: 8000
  },
  {
    timestamp: 97000,
    text: "Now I'll show chart sharing capabilities. Users can invite team members with specific permission levels: viewer access, commenting rights, or full editing privileges.",
    duration: 8000
  },
  {
    timestamp: 106000,
    text: "The collaboration history tracks all changes, comments, and annotations with full audit trails - essential for institutional compliance and research integrity.",
    duration: 8000
  },
  {
    timestamp: 115000,
    text: "Multiple team members can work simultaneously on the same economic analysis, with real-time synchronization ensuring everyone sees the latest insights and discussions.",
    duration: 8000
  },
  {
    timestamp: 124000,
    text: "The system supports professional workflows: analysts can add technical annotations, economists can provide contextual comments, and managers can pin critical insights.",
    duration: 8000
  },
  {
    timestamp: 133000,
    text: "All collaboration features work seamlessly across desktop, tablet, and mobile devices, enabling economic teams to collaborate from anywhere in the world.",
    duration: 7000
  },
  {
    timestamp: 141000,
    text: "This represents the future of collaborative economic analysis - combining Bloomberg Terminal-level functionality with modern real-time collaboration technology.",
    duration: 7000
  },
  {
    timestamp: 149000,
    text: "Thank you for exploring EconGraph's collaboration platform. Transform your economic analysis workflow with professional-grade real-time collaboration tools.",
    duration: 7000
  }
];

// Utility functions
async function typeWithDelay(page, selector, text, delay = DEMO_CONFIG.typingDelay) {
  await page.fill(selector, '');
  for (const char of text) {
    await page.type(selector, char, { delay });
  }
}

async function highlightElement(page, selector, duration = 2000) {
  await page.evaluate((sel) => {
    const element = document.querySelector(sel);
    if (element) {
      element.style.outline = '3px solid #ff4444';
      element.style.outlineOffset = '2px';
      setTimeout(() => {
        element.style.outline = '';
        element.style.outlineOffset = '';
      }, duration);
    }
  }, selector);
  await page.waitForTimeout(duration);
}

async function addNarrationOverlay(page, text, duration) {
  // Add narration text overlay
  await page.evaluate((narrationText, displayDuration) => {
    // Remove existing narration
    const existing = document.querySelector('.narration-overlay');
    if (existing) existing.remove();

    // Create narration overlay
    const overlay = document.createElement('div');
    overlay.className = 'narration-overlay';
    overlay.style.cssText = `
      position: fixed;
      top: 20px;
      left: 20px;
      right: 20px;
      background: rgba(0, 0, 0, 0.85);
      color: white;
      padding: 16px 24px;
      border-radius: 8px;
      font-size: 16px;
      font-weight: 500;
      z-index: 10000;
      box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
      backdrop-filter: blur(10px);
      border: 1px solid rgba(255, 255, 255, 0.1);
    `;
    overlay.textContent = narrationText;

    document.body.appendChild(overlay);

    // Remove after duration
    setTimeout(() => {
      if (overlay.parentNode) {
        overlay.remove();
      }
    }, displayDuration);
  }, text, duration);

  await page.waitForTimeout(duration);
}

async function simulateCollaborationActivity(page) {
  // Simulate multiple users being online
  await page.evaluate(() => {
    // Mock real-time collaboration data
    const mockCollaborators = [
      { id: '1', name: 'Sarah Chen', avatar: null, isOnline: true, role: 'editor' },
      { id: '2', name: 'Michael Rodriguez', avatar: null, isOnline: true, role: 'viewer' },
      { id: '3', name: 'Dr. Emily Watson', avatar: null, isOnline: false, role: 'owner' },
      { id: '4', name: 'James Park', avatar: null, isOnline: true, role: 'editor' },
    ];

    // Add to window for demo purposes
    window.mockCollaborators = mockCollaborators;
  });
}

async function createCollaborationDemo() {
  console.log('🎬 Starting Collaboration-Focused Demo Recording...');

  const browser = await chromium.launch({
    headless: false,
    args: ['--no-sandbox', '--disable-web-security']
  });

  const context = await browser.newContext({
    viewport: { width: DEMO_CONFIG.width, height: DEMO_CONFIG.height },
    recordVideo: {
      dir: path.dirname(DEMO_CONFIG.videoPath),
      size: { width: DEMO_CONFIG.width, height: DEMO_CONFIG.height }
    }
  });

  const page = await context.newPage();

  try {
    console.log('📱 Setting up demo environment...');

    // Navigate to the application
    await page.goto(DEMO_CONFIG.baseUrl);
    await page.waitForLoadState('networkidle');

    // Introduction narration
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[0].text, COLLABORATION_NARRATION_SCRIPT[0].duration);

    console.log('🤝 Showcasing Collaboration Workspace...');

    // Show collaboration workspace
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[1].text, COLLABORATION_NARRATION_SCRIPT[1].duration);

    // Navigate to a chart page with collaboration features
    await page.goto(`${DEMO_CONFIG.baseUrl}/series/GDPC1`);
    await page.waitForLoadState('networkidle');

    // Simulate collaboration activity
    await simulateCollaborationActivity(page);

    // Show active collaborators
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[2].text, COLLABORATION_NARRATION_SCRIPT[2].duration);

    // Try to open collaboration panel
    const collaborationButton = await page.locator('button:has-text("Collaboration"), [aria-label*="collaboration"], [title*="collaboration"]').first();
    if (await collaborationButton.isVisible()) {
      await collaborationButton.click();
      await page.waitForTimeout(1000);
    }

    console.log('📝 Demonstrating Chart Annotations...');

    // Annotation creation
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[3].text, COLLABORATION_NARRATION_SCRIPT[3].duration);

    // Try to create annotation
    const addAnnotationButton = await page.locator('button:has-text("Add Annotation"), button:has-text("Annotate")').first();
    if (await addAnnotationButton.isVisible()) {
      await addAnnotationButton.click();
      await page.waitForTimeout(1000);

      // Fill annotation form
      await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[4].text, COLLABORATION_NARRATION_SCRIPT[4].duration);

      // Try to fill form fields
      await page.fill('input[label*="Title"], input[placeholder*="Title"]', 'COVID-19 Economic Impact');
      await page.waitForTimeout(500);

      await page.fill('textarea[label*="Description"], textarea[placeholder*="Description"]', 'Major economic disruption due to pandemic lockdowns and Federal Reserve emergency response measures.');
      await page.waitForTimeout(1000);

      await page.fill('input[type="date"]', '2020-03-15');
      await page.waitForTimeout(500);

      // Select annotation type
      const typeSelect = await page.locator('select, [role="combobox"]').first();
      if (await typeSelect.isVisible()) {
        await typeSelect.click();
        await page.waitForTimeout(500);
        await page.click('option:has-text("Vertical Line"), [role="option"]:has-text("line")');
      }

      await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[5].text, COLLABORATION_NARRATION_SCRIPT[5].duration);

      // Submit annotation
      const submitButton = await page.locator('button:has-text("Add"), button:has-text("Create"), button:has-text("Save")').first();
      if (await submitButton.isVisible()) {
        await submitButton.click();
        await page.waitForTimeout(1000);
      }
    }

    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[6].text, COLLABORATION_NARRATION_SCRIPT[6].duration);

    console.log('💬 Demonstrating Comment System...');

    // Comment system demonstration
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[7].text, COLLABORATION_NARRATION_SCRIPT[7].duration);

    // Try to add comment
    const commentButton = await page.locator('button:has-text("Comment"), [aria-label*="comment"]').first();
    if (await commentButton.isVisible()) {
      await commentButton.click();
      await page.waitForTimeout(1000);

      await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[8].text, COLLABORATION_NARRATION_SCRIPT[8].duration);

      // Fill comment
      const commentField = await page.locator('textarea[placeholder*="comment"], input[placeholder*="comment"]').first();
      if (await commentField.isVisible()) {
        await typeWithDelay(page, 'textarea[placeholder*="comment"], input[placeholder*="comment"]', 'The Fed responded with unprecedented monetary policy measures including zero interest rates and quantitative easing.');

        const submitComment = await page.locator('button:has-text("Comment"), button:has-text("Post"), button:has-text("Add Comment")').first();
        if (await submitComment.isVisible()) {
          await submitComment.click();
          await page.waitForTimeout(1000);
        }
      }
    }

    console.log('🔧 Showing Annotation Management...');

    // Annotation management features
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[9].text, COLLABORATION_NARRATION_SCRIPT[9].duration);

    // Show filtering options
    const filterSelect = await page.locator('select:has(option:has-text("All")), [role="combobox"]:has-text("Filter")').first();
    if (await filterSelect.isVisible()) {
      await filterSelect.click();
      await page.waitForTimeout(1000);
      await highlightElement(page, 'option:has-text("All"), [role="option"]:has-text("All")');
      await page.click('option:has-text("Mine"), [role="option"]:has-text("Mine")');
      await page.waitForTimeout(1000);
    }

    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[10].text, COLLABORATION_NARRATION_SCRIPT[10].duration);

    console.log('👥 Demonstrating Permission System...');

    // Permission system
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[11].text, COLLABORATION_NARRATION_SCRIPT[11].duration);

    // Show sharing features
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[12].text, COLLABORATION_NARRATION_SCRIPT[12].duration);

    const shareButton = await page.locator('button:has-text("Share"), [aria-label*="share"]').first();
    if (await shareButton.isVisible()) {
      await shareButton.click();
      await page.waitForTimeout(2000);

      // Close share dialog
      const closeButton = await page.locator('button:has-text("Close"), button:has-text("Cancel")').first();
      if (await closeButton.isVisible()) {
        await closeButton.click();
      }
    }

    console.log('📊 Highlighting Professional Features...');

    // Professional workflow features
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[13].text, COLLABORATION_NARRATION_SCRIPT[13].duration);

    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[14].text, COLLABORATION_NARRATION_SCRIPT[14].duration);

    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[15].text, COLLABORATION_NARRATION_SCRIPT[15].duration);

    // Mobile responsiveness mention
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[16].text, COLLABORATION_NARRATION_SCRIPT[16].duration);

    // Future of collaboration
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[17].text, COLLABORATION_NARRATION_SCRIPT[17].duration);

    // Closing
    await addNarrationOverlay(page, COLLABORATION_NARRATION_SCRIPT[18].text, COLLABORATION_NARRATION_SCRIPT[18].duration);

    console.log('✅ Demo recording completed successfully!');

  } catch (error) {
    console.error('❌ Demo recording failed:', error);
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

// Export for use in other scripts
module.exports = {
  createCollaborationDemo,
  COLLABORATION_NARRATION_SCRIPT,
  DEMO_CONFIG
};

// Run if called directly
if (require.main === module) {
  createCollaborationDemo()
    .then(() => {
      console.log('🎉 Collaboration demo completed successfully!');
      process.exit(0);
    })
    .catch((error) => {
      console.error('💥 Demo failed:', error);
      process.exit(1);
    });
}
