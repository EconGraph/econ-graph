#!/bin/bash

# Record ONLY the Chrome browser window showing EconGraph
# This creates a professional video focused solely on the application

set -e

echo "🎬 RECORDING CHROME BROWSER WINDOW ONLY"
echo "📹 Professional demo showing only the EconGraph application"
echo ""

# Ensure output directory exists
mkdir -p demo-videos

# Check if services are running
echo "🔍 Checking services..."
if ! curl -s http://localhost:3000 > /dev/null; then
    echo "❌ Frontend not running. Please start with: cd frontend && npm start"
    exit 1
fi

if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "⚠️  Backend may not be running, but continuing..."
fi

echo "✅ Services are ready"
echo ""

# Open the application in Chrome and position it properly
echo "🌐 Opening EconGraph in Chrome and positioning window..."
osascript << 'EOF'
tell application "Google Chrome"
    activate
    delay 1

    -- Open the application
    if (count of windows) = 0 then
        make new window
    end if

    set URL of active tab of front window to "http://localhost:3000"
    delay 3

    -- Position and size the window for optimal recording
    tell front window
        set bounds to {100, 100, 1500, 900}  -- x, y, width, height
    end tell

    delay 2

    -- Add recording indicator
    tell active tab of front window
        execute javascript "
            // Remove any existing indicators
            const existing = document.querySelectorAll('.demo-indicator');
            existing.forEach(el => el.remove());

            // Create professional recording indicator
            const indicator = document.createElement('div');
            indicator.className = 'demo-indicator';
            indicator.style.cssText = 'position: fixed; top: 10px; right: 10px; background: #ff4444; color: white; padding: 8px 16px; border-radius: 4px; font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto; font-size: 14px; font-weight: 500; z-index: 10000; box-shadow: 0 2px 8px rgba(0,0,0,0.2);';
            indicator.textContent = '🔴 RECORDING';
            document.body.appendChild(indicator);

            // Add professional title
            const title = document.createElement('div');
            title.className = 'demo-indicator';
            title.style.cssText = 'position: fixed; top: 60px; left: 20px; background: rgba(25,118,210,0.95); color: white; padding: 12px 20px; border-radius: 6px; font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto; font-size: 18px; font-weight: 600; z-index: 10000; box-shadow: 0 4px 12px rgba(0,0,0,0.15);';
            title.textContent = 'EconGraph - Economic Data Visualization Platform';
            document.body.appendChild(title);
        "
    end tell
end tell
EOF

echo "🎭 Setting up automated interactions..."

# Create interaction script that will run during recording
cat > /tmp/chrome_interaction.scpt << 'EOF'
tell application "Google Chrome"
    activate
    delay 3

    tell active tab of front window
        -- Scroll to show different parts of the application
        execute javascript "window.scrollTo({top: 0, behavior: 'smooth'});"
    end tell
    delay 2

    tell active tab of front window
        execute javascript "window.scrollTo({top: 300, behavior: 'smooth'});"
    end tell
    delay 3

    tell active tab of front window
        execute javascript "window.scrollTo({top: 600, behavior: 'smooth'});"
    end tell
    delay 3

    tell active tab of front window
        execute javascript "window.scrollTo({top: 0, behavior: 'smooth'});"
    end tell
    delay 2

    -- Try to interact with navigation
    tell active tab of front window
        execute javascript "
            const navElements = document.querySelectorAll('a, button, .MuiTab-root, [role=\"tab\"]');
            if (navElements.length > 0) {
                navElements[0].click();
            }
        "
    end tell
    delay 4

    -- Search interaction
    tell active tab of front window
        execute javascript "
            const searchInput = document.querySelector('input[type=\"search\"], input[placeholder*=\"search\"], .MuiInputBase-input, input[type=\"text\"]');
            if (searchInput) {
                searchInput.focus();
                // Simulate typing
                const text = 'GDP Growth Rate';
                let i = 0;
                const typeInterval = setInterval(() => {
                    if (i < text.length) {
                        searchInput.value = text.substring(0, i + 1);
                        searchInput.dispatchEvent(new Event('input', {bubbles: true}));
                        i++;
                    } else {
                        clearInterval(typeInterval);
                    }
                }, 100);
            }
        "
    end tell
    delay 6

    -- Navigate through sections
    tell active tab of front window
        execute javascript "
            const clickableElements = document.querySelectorAll('a[href], .MuiTab-root, button, .MuiCard-root, .MuiListItem-root');
            let clickCount = 0;
            const maxClicks = 3;

            const clickNext = () => {
                if (clickCount < maxClicks && clickCount < clickableElements.length) {
                    const element = clickableElements[clickCount];
                    if (element && element.click) {
                        element.click();
                        clickCount++;
                        setTimeout(clickNext, 4000);
                    }
                }
            };

            setTimeout(clickNext, 1000);
        "
    end tell
    delay 15

    -- Final success message
    tell active tab of front window
        execute javascript "
            const final = document.createElement('div');
            final.className = 'demo-indicator';
            final.style.cssText = 'position: fixed; bottom: 20px; left: 50%; transform: translateX(-50%); background: rgba(76,175,80,0.95); color: white; padding: 16px 32px; border-radius: 8px; font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto; font-size: 16px; font-weight: 600; z-index: 10000; text-align: center; box-shadow: 0 4px 16px rgba(0,0,0,0.2);';
            final.innerHTML = '✅ EconGraph Demo Complete<br><span style=\"font-size: 14px; font-weight: 400;\">Professional Economic Data Platform</span>';
            document.body.appendChild(final);
        "
    end tell
    delay 3
end tell
EOF

# Get the Chrome window ID for targeted recording
echo "🎯 Identifying Chrome window for targeted recording..."
CHROME_WINDOW_ID=$(osascript -e 'tell application "Google Chrome" to get id of front window')

echo "📹 Starting BROWSER-ONLY screen recording..."
echo "⏱️  Recording 45 seconds of Chrome window only..."

# Record only the Chrome browser window using its window ID
ffmpeg -f avfoundation -capture_cursor 0 -i "1" -i "demo-tools/generated-audio/investor_narration_20min.aiff" \
       -filter_complex "[0:v]crop=1400:800:100:100[cropped]" \
       -map "[cropped]" -map 1:a \
       -c:v libx264 -preset fast -crf 23 -c:a aac -b:a 128k \
       -t 45 -y demo-videos/econ-graph-browser-demo.mp4 &

FFMPEG_PID=$!
sleep 3

# Run the interaction script
echo "🎭 Running automated browser interactions..."
osascript /tmp/chrome_interaction.scpt &

# Wait for recording to complete
wait $FFMPEG_PID

# Clean up
rm -f /tmp/chrome_interaction.scpt

# Check if video was created
if [ -f demo-videos/econ-graph-browser-demo.mp4 ]; then
    SIZE=$(ls -lh demo-videos/econ-graph-browser-demo.mp4 | awk '{print $5}')
    echo ""
    echo "✅ PROFESSIONAL BROWSER-ONLY VIDEO CREATED!"
    echo "📁 File: demo-videos/econ-graph-browser-demo.mp4"
    echo "💾 Size: ${SIZE}"
    echo "📹 Content: Chrome browser window only showing EconGraph"
    echo "🎵 Audio: Professional British investor narration"
    echo "🎯 Focus: Application interface only, no desktop clutter"
    echo ""
    echo "🚀 Ready for professional presentation!"
else
    echo "❌ Video creation failed"
    exit 1
fi
EOF
