# Global Analysis Features

## Overview

Global Analysis (`/global`) compares countries on World Bank development indicators through
an interactive world map. Geographic patterns help readers identify differences to explore
in economic research, policy discussions, and teaching. Countries with values link to their
underlying series for closer examination over time.

Release builds show the **World Map** tab. The additional tabs are disabled in release
builds; see [build flags](../../frontend/docs/build-flags.md) for the developer setting.
For step-by-step instructions, see the [Global Analysis User Guide](../user-guides/GLOBAL_ANALYSIS_USER_GUIDE.md).

## Economic data and comparison

The **Indicator** list offers the curated World Development Indicators available in the
application's World Bank dataset. The initial indicator is GDP per capita
(`NY.GDP.PCAP.CD`) when available, otherwise the first indicator in the list. Units belong
to the selected indicator and appear with the values when available.

Each country contributes its latest available observation. Countries can therefore have
values from different periods: this is a comparison of latest values, not necessarily a
comparison for a common year. The tooltip and values table show individual observation
dates, and the legend gives the earliest and latest dates among countries drawn with data.
These dates describe the observations, not when the application fetched them.

### Reading colors and coverage

The map uses a sequential color scale from the lowest to the highest value among countries
it can draw. Extreme values remain part of that range, so a few high or low values can make
smaller differences harder to distinguish. Changing the palette changes the appearance,
while retaining the same low-to-high ordering.

The legend names the indicator, shows its unit when available, and displays the value range
and number of countries with data on the map. Countries without a mapped value use the
**No data** color. Some countries have values but cannot be matched to a shape on the map;
the legend counts these separately, and their values remain available in the table.

### Color schemes

- **Viridis** (default): a purple-to-green-to-yellow progression for continuous values.
- **Blues**: light blue for lower values, dark blue for higher values.
- **Reds**: light red for lower values, dark red for higher values.
- **Greens**: light green for lower values, dark green for higher values.

The palette does not label values as positive, negative, desirable, or undesirable. Read the
indicator's meaning and unit alongside the colors when interpreting a pattern.

## Interactive world map

Hover over a country to see its name, value, unit when available, and observation date.
Click a country with data to open its series page. On a touch screen, the first tap shows
the tooltip and a second tap on that country opens the series.

Use the mouse wheel to zoom and drag to pan. The labeled **Zoom in**, **Zoom out**, and
**Reset zoom** buttons provide another way to adjust the view; the current zoom percentage
appears beside them. Reset restores the initial zoom and position.

### Map projections

The **Projection** control offers three views of the same country values:

- **Natural Earth** (default): a rounded world view for an overview of geographic patterns.
- **Mercator**: a rectangular view that enlarges areas toward the poles.
- **Orthographic**: a globe-like view of one hemisphere.

Projection choice changes the map's geometry, not the indicator or its values.

## Values table and responsive layout

Expand **Values as a table** below the map to compare countries numerically, highest value
first. The table has Country, Value, and Date columns, with units in the value heading when
available. Every country name links to its series, including countries with values that
have no shape on the map.

The table provides a keyboard and screen-reader route to country values and series links.
The zoom buttons also have accessible labels. Readers can use these controls and the table
without relying on country hover or color differences to obtain the values.

The map scales to its container, and the indicator and presentation controls wrap to fit
the available width. On narrower screens the legend moves below the map panel; on phones,
the zoom controls sit below the map so they do not cover countries.

## Loading and request messages

A progress bar indicates when the indicator list or country values are loading. While a
newly chosen indicator loads, the map, legend, and table keep showing the preceding
indicator's data; the legend continues to name the indicator being displayed.

If the indicator list or values fail to load, an error message offers **Retry**. Informational
messages distinguish an unavailable World Development Indicators dataset from a selected
indicator with no country values on the map. If the map outline fails to load, its error
message offers **Reload**.

## Explore a geographic pattern

1. Open **Global Analysis** and choose an **Indicator**.
2. Read the legend's unit and date range before comparing countries.
3. Choose a projection and palette, then zoom or pan to explore a region.
4. Hover over a country, or expand **Values as a table**, to inspect its value and date.
5. Open a country's series through the map or its table link to examine the underlying data
   over time.

This workflow connects a broad geographic view with the individual series needed to
investigate a difference, while keeping differences in observation dates visible.
