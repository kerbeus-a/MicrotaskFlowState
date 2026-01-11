import { useState } from "react";
import "./TaskList.css";

interface Task {
  id: number;
  text: string;
  completed: boolean;
  created_at: string;
  completed_at: string | null;
}

interface TaskListProps {
  tasks: Task[];
  onToggle: (id: number) => void;
  onDelete: (id: number) => void;
  onUpdate: (id: number, text: string) => void;
}

export default function TaskList({ tasks, onToggle, onDelete, onUpdate }: TaskListProps) {
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editText, setEditText] = useState("");

  const handleDoubleClick = (task: Task) => {
    setEditingId(task.id);
    setEditText(task.text);
  };

  const handleEditSubmit = (id: number) => {
    if (editText.trim()) {
      onUpdate(id, editText.trim());
    }
    setEditingId(null);
    setEditText("");
  };

  const handleEditCancel = () => {
    setEditingId(null);
    setEditText("");
  };

  const handleKeyDown = (e: React.KeyboardEvent, id: number) => {
    if (e.key === "Enter") {
      handleEditSubmit(id);
    } else if (e.key === "Escape") {
      handleEditCancel();
    }
  };

  const activeTasks = tasks.filter(t => !t.completed);
  const completedTasks = tasks.filter(t => t.completed);

  return (
    <div className="task-list">
      {activeTasks.length === 0 && completedTasks.length === 0 && (
        <div className="empty-state">
          <p>No tasks yet. Press the record button or Win+Alt+R to log your first task.</p>
        </div>
      )}
      
      {activeTasks.map((task) => (
        <div
          key={task.id}
          className={`task-item ${task.completed ? "completed" : ""}`}
        >
          <input
            type="checkbox"
            checked={task.completed}
            onChange={() => onToggle(task.id)}
            className="task-checkbox"
          />
          {editingId === task.id ? (
            <input
              type="text"
              value={editText}
              onChange={(e) => setEditText(e.target.value)}
              onBlur={() => handleEditSubmit(task.id)}
              onKeyDown={(e) => handleKeyDown(e, task.id)}
              className="task-edit-input"
              autoFocus
            />
          ) : (
            <span
              className="task-text"
              onDoubleClick={() => handleDoubleClick(task)}
            >
              {task.text}
            </span>
          )}
          <button
            className="task-delete"
            onClick={() => onDelete(task.id)}
            title="Delete task"
          >
            ×
          </button>
        </div>
      ))}
      
      {completedTasks.length > 0 && (
        <div className="completed-section">
          <h3>Completed</h3>
          {completedTasks.map((task) => (
            <div
              key={task.id}
              className="task-item completed"
            >
              <input
                type="checkbox"
                checked={task.completed}
                onChange={() => onToggle(task.id)}
                className="task-checkbox"
              />
              <span className="task-text">{task.text}</span>
              <button
                className="task-delete"
                onClick={() => onDelete(task.id)}
                title="Delete task"
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
