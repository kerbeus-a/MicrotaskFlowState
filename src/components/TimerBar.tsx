import "./TimerBar.css";

interface TimerBarProps {
  remaining: number;
}

export default function TimerBar({ remaining }: TimerBarProps) {
  const totalSeconds = 15 * 60; // 15 minutes
  const progress = (remaining / totalSeconds) * 100;
  const minutes = Math.floor(remaining / 60);
  const seconds = remaining % 60;

  return (
    <div className="timer-bar">
      <div className="timer-progress" style={{ width: `${progress}%` }} />
      <div className="timer-text">
        {minutes}:{seconds.toString().padStart(2, "0")}
      </div>
    </div>
  );
}
