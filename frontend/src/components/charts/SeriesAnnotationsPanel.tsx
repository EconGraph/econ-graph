import React from 'react';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  List,
  ListItemButton,
  ListItemText,
  Skeleton,
  Typography,
} from '@mui/material';
import { Comment as CommentIcon } from '@mui/icons-material';
import type { SeriesAnnotation } from '../../hooks/useSeriesAnnotations';
import { formatIsoDate } from '../../utils/dates';

export interface SeriesAnnotationsPanelProps {
  annotations: readonly SeriesAnnotation[];
  isLoading?: boolean;
  /** Load error; the list is replaced by a message with a retry button. */
  error?: Error | null;
  onRetry?: () => void;
  /** Highlighted annotation, for example the one clicked on the chart. */
  selectedId?: string | null;
  /** Called with an annotation's id when its row is clicked. */
  onSelect?: (id: string) => void;
  /** Extra controls in the panel header, such as an "Add annotation" button. */
  headerAction?: React.ReactNode;
  /** Extra content under an annotation's row, such as edit controls or comments. */
  renderAnnotationExtra?: (annotation: SeriesAnnotation) => React.ReactNode;
}

/**
 * Read-only list of a series' annotations, shown beside its chart. Signed-in features
 * (create, edit, comments) plug in through `headerAction` and `renderAnnotationExtra`.
 * @param props - See `SeriesAnnotationsPanelProps`.
 * @returns The panel.
 */
const SeriesAnnotationsPanel: React.FC<SeriesAnnotationsPanelProps> = ({
  annotations,
  isLoading = false,
  error = null,
  onRetry,
  selectedId = null,
  onSelect,
  headerAction,
  renderAnnotationExtra,
}) => {
  const list = React.useRef<React.ElementRef<'ul'>>(null);
  const rows = React.useRef(new Map<string, HTMLElement>());

  // Bring an annotation picked on the chart into view in the list. Only the list scrolls,
  // so the page stays on the chart that was clicked. Also re-run when the rows remount or
  // reorder (after loading or an error), since the selection may have changed meanwhile.
  React.useEffect(() => {
    const container = list.current;
    const row = selectedId ? rows.current.get(selectedId) : undefined;
    if (!container || !row) return;
    // The list is positioned, so it is each row's offsetParent.
    const top = row.offsetTop;
    if (top < container.scrollTop) {
      container.scrollTop = top;
    } else if (top + row.offsetHeight > container.scrollTop + container.clientHeight) {
      container.scrollTop = top + row.offsetHeight - container.clientHeight;
    }
  }, [selectedId, annotations, isLoading, error]);

  let body: React.ReactNode;
  if (isLoading) {
    body = <Skeleton variant='rectangular' height={80} data-testid='annotations-loading' />;
  } else if (error) {
    body = (
      <Alert
        severity='error'
        action={
          onRetry && (
            <Button color='inherit' size='small' onClick={onRetry}>
              Retry
            </Button>
          )
        }
      >
        Could not load annotations. {error.message}
      </Alert>
    );
  } else if (annotations.length === 0) {
    body = (
      <Typography variant='body2' color='text.secondary'>
        No public annotations on this series yet.
      </Typography>
    );
  } else {
    body = (
      <List
        ref={list}
        dense
        disablePadding
        aria-label='Annotations'
        sx={{ maxHeight: 400, overflowY: 'auto', position: 'relative' }}
      >
        {annotations.map(annotation => (
          <Box
            component='li'
            key={annotation.id}
            ref={(el: HTMLElement | null) => {
              if (el) rows.current.set(annotation.id, el);
              else rows.current.delete(annotation.id);
            }}
            sx={{ listStyle: 'none' }}
          >
            <ListItemButton
              selected={annotation.id === selectedId}
              aria-current={annotation.id === selectedId ? 'true' : undefined}
              onClick={() => onSelect?.(annotation.id)}
              sx={{
                borderLeft: 4,
                borderColor: annotation.color ?? 'warning.main',
                alignItems: 'flex-start',
              }}
            >
              <ListItemText
                primary={annotation.title}
                secondary={
                  <>
                    <Typography component='span' variant='caption' display='block'>
                      {formatIsoDate(annotation.date)}
                    </Typography>
                    {annotation.description}
                  </>
                }
              />
            </ListItemButton>
            {renderAnnotationExtra?.(annotation)}
          </Box>
        ))}
      </List>
    );
  }

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
          <Typography variant='h6' component='h2' sx={{ display: 'flex', alignItems: 'center' }}>
            <CommentIcon sx={{ mr: 1 }} />
            Annotations
          </Typography>
          {headerAction && <Box sx={{ ml: 'auto' }}>{headerAction}</Box>}
        </Box>
        {body}
      </CardContent>
    </Card>
  );
};

export default SeriesAnnotationsPanel;
