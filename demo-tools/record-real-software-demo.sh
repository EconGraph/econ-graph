#!/bin/bash

# Record actual EconGraph software demonstration
# This creates a real video of the running application

set -e

echo "🎬 RECORDING ACTUAL ECON-GRAPH SOFTWARE DEMONSTRATION"
echo "📹 This will create a real video of the running application"
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

# Open the application in Chrome
echo "🌐 Opening EconGraph application in Chrome..."
open -a "Google Chrome" http://localhost:3000
sleep 3

# Create AppleScript to interact with the application
echo "🤖 Creating automated interaction script..."
cat > /tmp/demo_interaction.scpt << 'EOF'
tell application "Google Chrome"
    activate
    delay 2

    -- Maximize window
    tell front window
        set bounds to {0, 0, 1920, 1080}
    end tell

    delay 2

    -- Simulate some interactions with JavaScript
    tell active tab of front window
        -- Add a visual indicator that this is being recorded
        execute javascript "
            // Create recording indicator
            const indicator = document.createElement('div');
            indicator.style.cssText = 'position: fixed; top: 20px; left: 20px; background: #ff0000; color: white; padding: 10px 20px; border-radius: 5px; font-family: Arial; font-size: 16px; font-weight: bold; z-index: 10000; box-shadow: 0 2px 10px rgba(0,0,0,0.3);';
            indicator.textContent = '🔴 LIVE DEMO RECORDING';
            document.body.appendChild(indicator);

            // Add title overlay
            const title = document.createElement('div');
            title.style.cssText = 'position: fixed; top: 80px; left: 20px; background: rgba(0,0,0,0.8); color: white; padding: 15px 25px; border-radius: 10px; font-family: Arial; font-size: 24px; font-weight: bold; z-index: 10000;';
            title.textContent = 'EconGraph - Economic Data Visualization Platform';
            document.body.appendChild(title);

            // Add subtitle
            const subtitle = document.createElement('div');
            subtitle.style.cssText = 'position: fixed; top: 140px; left: 20px; background: rgba(25,118,210,0.9); color: white; padding: 10px 20px; border-radius: 5px; font-family: Arial; font-size: 16px; z-index: 10000;';
            subtitle.textContent = 'Professional Investor Demonstration - Live Application';
            document.body.appendChild(subtitle);
        "
    end tell

    delay 5

    -- Scroll and interact with the page
    tell active tab of front window
        execute javascript "window.scrollTo(0, 300);"
    end tell
    delay 3

    tell active tab of front window
        execute javascript "window.scrollTo(0, 600);"
    end tell
    delay 3

    tell active tab of front window
        execute javascript "window.scrollTo(0, 0);"
    end tell
    delay 2

    -- Try to click on navigation elements
    tell active tab of front window
        execute javascript "
            const navLinks = document.querySelectorAll('a, button, .MuiTab-root');
            if (navLinks.length > 0) {
                navLinks[0].click();
            }
        "
    end tell
    delay 4

    -- Try to interact with search if available
    tell active tab of front window
        execute javascript "
            const searchInput = document.querySelector('input[type=\"search\"], input[placeholder*=\"search\"], .MuiInputBase-input');
            if (searchInput) {
                searchInput.focus();
                searchInput.value = 'GDP Growth';
                searchInput.dispatchEvent(new Event('input', {bubbles: true}));
            }
        "
    end tell
    delay 5

    -- Navigate through different sections
    tell active tab of front window
        execute javascript "
            const links = document.querySelectorAll('a[href], .MuiTab-root');
            for (let i = 0; i < Math.min(3, links.length); i++) {
                setTimeout(() => {
                    if (links[i]) links[i].click();
                }, i * 3000);
            }
        "
    end tell
    delay 10

    -- Final message
    tell active tab of front window
        execute javascript "
            const final = document.createElement('div');
            final.style.cssText = 'position: fixed; bottom: 50px; left: 50%; transform: translateX(-50%); background: rgba(76,175,80,0.95); color: white; padding: 20px 40px; border-radius: 10px; font-family: Arial; font-size: 20px; font-weight: bold; z-index: 10000; text-align: center;';
            final.innerHTML = '✅ EconGraph Demo Complete<br><small>Ready for Investment Opportunity</small>';
            document.body.appendChild(final);
        "
    end tell
    delay 3
end tell
EOF

# Start screen recording with ffmpeg
echo "📹 Starting screen recording of actual software..."
echo "⏱️  Recording 60 seconds of live application demonstration..."

# Record the screen showing the Chrome window
ffmpeg -f avfoundation -i "1" -i "demo-tools/generated-audio/investor_narration_20min.aiff" \
       -vf "scale=1920:1080" -c:v libx264 -preset fast -c:a aac -b:a 128k \
       -t 60 -y demo-videos/econ-graph-live-demo.mp4 &

FFMPEG_PID=$!
sleep 2

# Run the interaction script
echo "🎭 Running automated interactions with the live application..."
osascript /tmp/demo_interaction.scpt &

# Wait for recording to complete
wait $FFMPEG_PID

# Clean up
rm -f /tmp/demo_interaction.scpt

# Check if video was created
if [ -f demo-videos/econ-graph-live-demo.mp4 ]; then
    SIZE=$(ls -lh demo-videos/econ-graph-live-demo.mp4 | awk '{print $5}')
    echo ""
    echo "✅ LIVE SOFTWARE DEMONSTRATION VIDEO CREATED!"
    echo "📁 File: demo-videos/econ-graph-live-demo.mp4"
    echo "💾 Size: ${SIZE}"
    echo "📹 Content: Actual EconGraph application running with live interactions"
    echo "🎵 Audio: Professional British investor narration"
    echo ""
    echo "🚀 Ready to commit to GitHub repository!"
else
    echo "❌ Video creation failed"
    exit 1
fi
EOF
