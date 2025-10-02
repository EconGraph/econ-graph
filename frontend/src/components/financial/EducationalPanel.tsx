import React from 'react';

interface EducationalPanelProps {
  ratioName: string;
  formula: string;
  description: string;
  educationalLink?: string;
}

export const EducationalPanel = React.memo<EducationalPanelProps>(
  ({ ratioName, formula, description, educationalLink }) => {
    return (
      <div className='educational-panel bg-blue-50 border border-blue-200 rounded-lg p-4'>
        <h4 className='text-md font-semibold text-blue-800 mb-2'>{ratioName}</h4>
        <div className='space-y-2'>
          <div>
            <span className='text-sm font-medium text-blue-700'>Formula:</span>
            <p className='text-sm text-blue-600 font-mono'>{formula}</p>
          </div>
          <div>
            <span className='text-sm font-medium text-blue-700'>Description:</span>
            <p className='text-sm text-blue-600'>{description}</p>
          </div>
          {educationalLink && (
            <div>
              <a
                href={educationalLink}
                target='_blank'
                rel='noopener noreferrer'
                className='text-sm text-blue-600 hover:text-blue-800 underline'
              >
                Learn more about this ratio
              </a>
            </div>
          )}
        </div>
      </div>
    );
  }
);

EducationalPanel.displayName = 'EducationalPanel';
