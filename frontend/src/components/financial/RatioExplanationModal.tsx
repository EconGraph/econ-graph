import React from 'react';

interface RatioExplanationModalProps {
  isOpen: boolean;
  onClose: () => void;
  ratioName: string;
  formula: string;
  description: string;
  interpretation: string;
  educationalLink?: string;
}

export const RatioExplanationModal: React.FC<RatioExplanationModalProps> = ({
  isOpen,
  onClose,
  ratioName,
  formula,
  description,
  interpretation,
  educationalLink,
}) => {
  if (!isOpen) return null;

  return (
    <div className='fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50'>
      <div className='bg-white rounded-lg p-6 max-w-2xl w-full mx-4 max-h-[80vh] overflow-y-auto'>
        <div className='flex justify-between items-start mb-4'>
          <h2 className='text-xl font-semibold'>{ratioName}</h2>
          <button onClick={onClose} className='text-gray-500 hover:text-gray-700 text-2xl'>
            ×
          </button>
        </div>

        <div className='space-y-4'>
          <div>
            <h3 className='text-lg font-medium text-gray-800 mb-2'>Formula</h3>
            <p className='text-gray-600 font-mono bg-gray-100 p-2 rounded'>{formula}</p>
          </div>

          <div>
            <h3 className='text-lg font-medium text-gray-800 mb-2'>Description</h3>
            <p className='text-gray-600'>{description}</p>
          </div>

          <div>
            <h3 className='text-lg font-medium text-gray-800 mb-2'>Interpretation</h3>
            <p className='text-gray-600'>{interpretation}</p>
          </div>

          {educationalLink && (
            <div>
              <h3 className='text-lg font-medium text-gray-800 mb-2'>Learn More</h3>
              <a
                href={educationalLink}
                target='_blank'
                rel='noopener noreferrer'
                className='text-blue-600 hover:text-blue-800 underline'
              >
                External educational resource
              </a>
            </div>
          )}
        </div>

        <div className='mt-6 flex justify-end'>
          <button
            onClick={onClose}
            className='px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-700'
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
