# Global Analysis User Guide

The Global Analysis page (`/global`) shows one World Bank World Development Indicator at a
time on a world map: each country's latest value, colored on a scale. In release builds the
page has one tab, **World Map**. The comparison and events tabs are compiled out of release
builds by the `global_analysis_tabs` build flag (see
[build flags](../../frontend/docs/build-flags.md)).

## Choosing an indicator

Pick an indicator from the **Indicator** list above the map. The list holds the curated
World Development Indicators the crawler loads. Until you pick one, the map shows GDP per
capita (`NY.GDP.PCAP.CD`) when it is loaded, otherwise the first indicator in the list.

While the next indicator loads, the map, legend and table keep showing the previous one.

## Reading the map

- **Hover** over a country to see its name, its latest value with unit, and the date of
  that value.
- **Click** a country that has a value to open its series page. On a touch screen, the first
  tap shows the tooltip and a second tap opens the series.
- Countries with no value for the indicator are drawn in the "No data" color.
- The **legend** beside the map names the indicator and its unit, shows the color scale
  from the lowest to the highest value, how many countries have a value, the dates the
  values come from, and how many countries with a value have no shape on the map.

## Map controls

- **Zoom in**, **Zoom out** and **Reset zoom** buttons sit on the map, with the current zoom
  level. You can also zoom with the scroll wheel and pan by dragging.
- **Projection**: Natural Earth (the default), Mercator or Orthographic.
- **Color scheme**: Viridis (the default), Blues or Reds.

## The values table

Under the map, a collapsed table lists every country with a value, highest first, each
linking to its series. It works with a keyboard and a screen reader, and it includes
countries that have no shape on the map, such as Kosovo.

## Messages

| Message | Meaning |
| --- | --- |
| Couldn't load the list of indicators. | The indicator list request failed. Use **Retry** |
| World Development Indicators haven't been loaded yet, so the map has no data. | The crawler hasn't loaded the World Bank dataset in this deployment |
| Couldn't load *indicator*. | The values for the chosen indicator failed to load. Use **Retry** |
| No country has a value for *indicator* yet. | The indicator is in the list but has no observations yet |
