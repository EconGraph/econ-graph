#!/bin/bash

OUTPUT_FILE="demo-videos/professional-business-impact-demo.mp4"

echo "🏢 Creating Professional Business Impact VIDEO"
echo "💰 Bloomberg Terminal ($24k) vs EconGraph (FREE)"
echo "📁 Output will be: $OUTPUT_FILE"
echo ""

# Create a 90-second professional business impact video with text overlays
echo "🎬 Creating professional business impact video..."

ffmpeg -f lavfi -i color=c=0x1976d2:size=1920x1080:duration=90 \
    -vf "drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='EconGraph Professional Business Impact':fontsize=64:fontcolor=white:x=(w-text_w)/2:y=200:enable='between(t,0,8)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Bloomberg Terminal: \$24,000/year':fontsize=48:fontcolor=white:x=(w-text_w)/2:y=350:enable='between(t,0,8)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Thomson Reuters: \$22,000/year':fontsize=48:fontcolor=white:x=(w-text_w)/2:y=450:enable='between(t,0,8)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='S&P CapIQ: \$12,000/year':fontsize=48:fontcolor=white:x=(w-text_w)/2:y=550:enable='between(t,0,8)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='EconGraph: FREE':fontsize=64:fontcolor=yellow:x=(w-text_w)/2:y=650:enable='between(t,0,8)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Professional Dashboard':fontsize=56:fontcolor=white:x=(w-text_w)/2:y=300:enable='between(t,8,16)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Bloomberg Terminal-Level Interface':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=400:enable='between(t,8,16)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Real-Time Economic Indicators':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=500:enable='between(t,8,16)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Global Economic Network Analysis':fontsize=54:fontcolor=white:x=(w-text_w)/2:y=250:enable='between(t,16,30)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Interactive World Map Visualization':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=350:enable='between(t,16,30)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='D3.js-Powered Economic Correlations':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=450:enable='between(t,16,30)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Premium Terminal-Level Features':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=550:enable='between(t,16,30)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Multi-Country Dashboard':fontsize=56:fontcolor=white:x=(w-text_w)/2:y=300:enable='between(t,30,45)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Comparative Economic Analysis':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=400:enable='between(t,30,45)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Synchronized Charts & Correlations':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=500:enable='between(t,30,45)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Global Events Explorer':fontsize=56:fontcolor=white:x=(w-text_w)/2:y=250:enable='between(t,45,60)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='2008 Financial Crisis Tracking':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=350:enable='between(t,45,60)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='COVID-19 Economic Impact Analysis':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=450:enable='between(t,45,60)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Brexit & Policy Change Effects':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=550:enable='between(t,45,60)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Transformative Business Impact':fontsize=54:fontcolor=yellow:x=(w-text_w)/2:y=250:enable='between(t,60,75)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Hundreds of Thousands in Savings':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=350:enable='between(t,60,75)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Enterprise Capabilities at Zero Cost':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=450:enable='between(t,60,75)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Open-Source Competitive Advantage':fontsize=44:fontcolor=white:x=(w-text_w)/2:y=550:enable='between(t,60,75)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='EconGraph: The Future of Economic Analysis':fontsize=52:fontcolor=white:x=(w-text_w)/2:y=300:enable='between(t,75,90)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='Professional • Open-Source • Free':fontsize=48:fontcolor=yellow:x=(w-text_w)/2:y=450:enable='between(t,75,90)',\
         drawtext=fontfile='/System/Library/Fonts/ArialHB.ttc':text='github.com/EconGraph/econ-graph':fontsize=40:fontcolor=white:x=(w-text_w)/2:y=550:enable='between(t,75,90)'" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$OUTPUT_FILE" -y

if [ $? -eq 0 ]; then
    echo "✅ Professional business impact VIDEO created: $OUTPUT_FILE"
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    echo "📊 File size: ${FINAL_SIZE}"
    echo ""
    echo "🏢 PROFESSIONAL BUSINESS IMPACT VIDEO FEATURES:"
    echo "   ✅ 90-second professional business presentation"
    echo "   ✅ Bloomberg Terminal cost comparison ($24k vs FREE)"
    echo "   ✅ Thomson Reuters competitive analysis ($22k vs FREE)"
    echo "   ✅ S&P CapIQ functionality comparison ($12k vs FREE)"
    echo "   ✅ Professional feature highlights and capabilities"
    echo "   ✅ Business value proposition clearly presented"
    echo "   ✅ GitHub repository prominently featured"
    echo ""
    echo "💰 COST SAVINGS DEMONSTRATED:"
    echo "   📊 Bloomberg Terminal: $24,000/year → FREE"
    echo "   📊 Thomson Reuters: $22,000/year → FREE"
    echo "   📊 S&P CapIQ: $12,000/year → FREE"
    echo "   💎 Total Potential Savings: $58,000+/year per user"
    echo ""
    echo "🎬 Opening the professional business impact video..."
    open "$OUTPUT_FILE"
else
    echo "❌ Error creating professional business impact video."
    exit 1
fi
