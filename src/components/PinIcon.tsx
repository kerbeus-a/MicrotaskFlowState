import './PinIcon.css';

interface PinIconProps {
  isPinned: boolean;
  className?: string;
}

export default function PinIcon({ isPinned, className = '' }: PinIconProps) {
  return (
    <div className={`pin-toggle ${isPinned ? 'pinned' : ''} ${className}`}>
      <div className="pin-toggle-slider"></div>
    </div>
  );
}
