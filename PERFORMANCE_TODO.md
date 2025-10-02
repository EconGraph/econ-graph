# Frontend Performance Optimization Roadmap

**Base Branch:** `frontend/vitest-migration`  
**Status:** Draft - Work in Progress

---

## 🎯 Performance Optimization Tasks

### ✅ Completed
- [x] FinancialAlerts: Wrap event handlers in useCallback
- [x] FinancialAlerts tests: Reuse QueryClient across tests

### 🔄 High Priority - Production Impact

#### 1. Extract AlertCard to Memoized Component
**File:** `frontend/src/components/financial/FinancialAlerts.tsx`  
**Current Issue:** All alerts re-render when any alert changes  
**Solution:** Extract inline JSX (lines 390-540) to separate `React.memo(AlertCard)`  
**Impact:** O(1) re-renders instead of O(N) - critical for 10+ alerts

#### 2. Wrap Event Handlers in useCallback
**Files to fix:**
- `FinancialDashboard.tsx` - handleRefresh, handleExportData, handleShareAnalysis
- `TrendAnalysisChart.tsx` - All chart interaction handlers
- `PeerComparisonChart.tsx` - View mode changes, sorting handlers
- `FinancialStatementViewer.tsx` - Tab changes, line item selection

**Impact:** Prevents unnecessary child re-renders

#### 3. Add React.memo to Presentational Components
**Candidates:**
- `EducationalPanel` - Rarely changes, always same props
- `AnnotationPanel` - Only changes when annotations change
- `CollaborativePresence` - Only changes when collaborators change
- `BenchmarkComparison` - Only changes when benchmark data changes

**Impact:** Prevents re-renders when parent state changes

### 🔧 Medium Priority

#### 4. Optimize Data Transformations
- Review all `useMemo` dependency arrays
- Ensure expensive calculations are properly memoized
- Check for over-triggering (arrays/objects in deps)

#### 5. Test Suite Performance
**Apply QueryClient reuse pattern to:**
- `FinancialDashboard.test.tsx`
- `FinancialStatementViewer.test.tsx`
- `RatioAnalysisPanel.test.tsx`

**Impact:** 10-20% faster test execution

### 📦 Low Priority

#### 6. Virtual Scrolling
- Implement for alert lists with 50+ items
- Use `react-window` or `react-virtualized`
- Only render visible items in viewport

#### 7. Code Splitting
- Lazy load heavy financial components
- Split D3.js visualization bundles
- Reduce initial bundle size

---

## 📊 Performance Metrics to Track

### Before Optimization
- [ ] Measure component render counts with React DevTools Profiler
- [ ] Track test suite execution time baseline
- [ ] Identify slowest components

### After Optimization
- [ ] Verify reduced render counts
- [ ] Measure test suite improvement
- [ ] Confirm no functionality regressions

---

## 🔬 Testing Strategy

1. All optimizations must maintain 100% test coverage
2. Run tests with 1-second timeout to catch performance regressions
3. Use React DevTools Profiler to verify improvements
4. Benchmark before/after for key user flows

---

## 📝 Notes

- Prioritize optimizations that benefit BOTH production and tests
- Avoid premature optimization - focus on measured bottlenecks
- Keep changes incremental and testable
- Document performance gains in commit messages

