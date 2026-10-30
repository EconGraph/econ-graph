import type { Page } from '@playwright/test';

/**
 * Measure visible land, weighted by rendered area rather than one vote per country.
 * The country mask uses geometry and data-has-data only, never the rendered color:
 * white data-bearing land therefore stays in the sample and fails the white bound.
 */
export async function landColorDistribution(page: Page) {
  return page
    .getByTestId('world-map')
    .locator('svg[aria-label="World map"]')
    .evaluate(async element => {
      const svg = element as SVGSVGElement;
      const width = 960;
      const height = 500;
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext('2d')!;
      const mask = document.createElement('canvas');
      mask.width = width;
      mask.height = height;
      const maskContext = mask.getContext('2d')!;
      const paths = [...svg.querySelectorAll<SVGPathElement>('path.country')];
      const countries = paths.map(path => ({
        iso: Number(path.getAttribute('data-iso-numeric')),
        hasData: path.getAttribute('data-has-data') === 'true',
      }));
      const rootMatrix = svg.getCTM()!;
      paths.forEach((path, index) => {
        // Encode country index, independently of its fill, in an opaque geometry mask.
        const id = index + 1;
        maskContext.fillStyle = `rgb(${id & 255},${(id >> 8) & 255},0)`;
        const matrix = rootMatrix.inverse().multiply(path.getCTM()!);
        maskContext.setTransform(matrix);
        maskContext.fill(new Path2D(path.getAttribute('d') ?? ''));
      });
      const clone = svg.cloneNode(true) as SVGSVGElement;
      clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
      clone.setAttribute('width', String(width));
      clone.setAttribute('height', String(height));
      // Inline computed fills and opacity so the measurement includes actual styles.
      clone.querySelectorAll('path').forEach((path, i) => {
        const original = svg.querySelectorAll('path')[i];
        const style = getComputedStyle(original);
        path.style.setProperty('fill', style.fill, 'important');
        path.style.setProperty('stroke', style.stroke, 'important');
        path.style.setProperty('opacity', style.opacity, 'important');
      });
      const picture = new Image();
      picture.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(clone))}`;
      await picture.decode();
      context.fillStyle = getComputedStyle(svg).backgroundColor;
      context.fillRect(0, 0, width, height);
      context.drawImage(picture, 0, 0, width, height);
      const pixels = context.getImageData(0, 0, width, height).data;
      const geometry = maskContext.getImageData(0, 0, width, height).data;
      const idAt = (x: number, y: number) => {
        const offset = (y * width + x) * 4;
        return geometry[offset + 3] === 255 ? geometry[offset] + (geometry[offset + 1] << 8) : 0;
      };
      const linear = (channel: number) => {
        const value = channel / 255;
        return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
      };
      let landPixels = 0;
      let dataPixels = 0;
      let whitePixels = 0;
      let lightness = 0;
      const rgb = [0, 0, 0];
      const bands = [0, 0, 0];
      const countryPixels: Record<number, number> = {};
      for (let y = 2; y < height - 2; y++) {
        for (let x = 2; x < width - 2; x++) {
          const id = idAt(x, y);
          if (!id) continue;
          // Erode two pixels to avoid coast/border strokes and antialiasing.
          if (
            [
              [-2, 0],
              [2, 0],
              [0, -2],
              [0, 2],
            ].some(([dx, dy]) => idAt(x + dx, y + dy) !== id)
          )
            continue;
          landPixels++;
          const country = countries[id - 1];
          if (!country?.hasData) continue;
          dataPixels++;
          countryPixels[country.iso] = (countryPixels[country.iso] ?? 0) + 1;
          const offset = (y * width + x) * 4;
          const r = pixels[offset],
            g = pixels[offset + 1],
            b = pixels[offset + 2];
          rgb[0] += r;
          rgb[1] += g;
          rgb[2] += b;
          if (r >= 235 && g >= 235 && b >= 235) whitePixels++;
          // CIELAB L* from sRGB luminance: arithmetic RGB distance is not perceptual.
          const luminance = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
          const l =
            luminance > 216 / 24389 ? 116 * Math.cbrt(luminance) - 16 : (24389 / 27) * luminance;
          lightness += l;
          bands[l < 45 ? 0 : l < 70 ? 1 : 2]++;
        }
      }
      return {
        dataPixels,
        landPixels,
        dataFraction: dataPixels / landPixels,
        noDataFraction: 1 - dataPixels / landPixels,
        averageRgb: rgb.map(channel => channel / dataPixels),
        meanLightness: lightness / dataPixels,
        lightnessBands: bands.map(count => count / dataPixels),
        nearWhiteFraction: whitePixels / dataPixels,
        countryPixels,
      };
    });
}
