import React from 'react';

interface TeamMember {
  id: string;
  name: string;
  avatar?: string;
  isActive: boolean;
  lastSeen?: string;
}

interface CollaborativePresenceProps {
  teamMembers: TeamMember[];
  currentUser: string;
}

export const CollaborativePresence = React.memo<CollaborativePresenceProps>(
  ({ teamMembers, currentUser }) => {
    return (
      <div className='collaborative-presence'>
        <h4 className='text-sm font-medium text-gray-700 mb-2'>Team Members</h4>
        <div className='flex flex-wrap gap-2'>
          {teamMembers.map(member => (
            <div
              key={member.id}
              className={`flex items-center gap-1 px-2 py-1 rounded-full text-xs ${
                member.isActive ? 'bg-green-100 text-green-800' : 'bg-gray-100 text-gray-600'
              }`}
            >
              <div
                className={`w-2 h-2 rounded-full ${member.isActive ? 'bg-green-500' : 'bg-gray-400'}`}
              />
              <span>{member.name}</span>
              {member.id === currentUser && <span className='text-gray-500'>(you)</span>}
            </div>
          ))}
        </div>
      </div>
    );
  }
);

CollaborativePresence.displayName = 'CollaborativePresence';
