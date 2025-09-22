import React from 'react';
import { FinancialAnnotation } from '@/types/financial';

interface AnnotationPanelProps {
  annotations: FinancialAnnotation[];
  onAddAnnotation: (content: string, type: string) => void;
  onUpdateAnnotation: (id: string, content: string) => void;
  onDeleteAnnotation: (id: string) => void;
}

export const AnnotationPanel: React.FC<AnnotationPanelProps> = ({
  annotations,
  onAddAnnotation,
  onUpdateAnnotation,
  onDeleteAnnotation,
}) => {
  return (
    <div className='annotation-panel'>
      <h3 className='text-lg font-semibold mb-4'>Annotations</h3>
      <div className='space-y-4'>
        {annotations.map(annotation => (
          <div key={annotation.id} className='border rounded-lg p-4'>
            <div className='flex justify-between items-start mb-2'>
              <span className='text-sm font-medium text-gray-600'>{annotation.annotationType}</span>
              <span className='text-xs text-gray-500'>
                {new Date(annotation.createdAt).toLocaleDateString()}
              </span>
            </div>
            <p className='text-sm text-gray-800 mb-2'>{annotation.content}</p>
            <div className='flex gap-2'>
              <button
                onClick={() => onUpdateAnnotation(annotation.id, annotation.content)}
                className='text-xs text-blue-600 hover:text-blue-800'
              >
                Edit
              </button>
              <button
                onClick={() => onDeleteAnnotation(annotation.id)}
                className='text-xs text-red-600 hover:text-red-800'
              >
                Delete
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
