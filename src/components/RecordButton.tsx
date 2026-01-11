import "./RecordButton.css";

interface RecordButtonProps {
  isRecording: boolean;
  onClick: () => void;
}

export default function RecordButton({ isRecording, onClick }: RecordButtonProps) {
  return (
    <div className="record-button-container">
      <button
        className={`record-button ${isRecording ? "recording" : ""}`}
        onClick={onClick}
        title={isRecording ? "Stop recording" : "Start recording (Win+Alt+R)"}
      >
        <svg
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {isRecording ? (
            <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" />
          ) : (
            <circle cx="12" cy="12" r="10" fill="currentColor" />
          )}
        </svg>
      </button>
      {isRecording && (
        <div className="recording-indicator">
          <span>Recording...</span>
        </div>
      )}
    </div>
  );
}
