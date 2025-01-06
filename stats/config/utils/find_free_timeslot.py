import tkinter as tk
from tkinter import ttk, filedialog, messagebox
import json
from datetime import datetime, timedelta
from croniter import croniter
import colorsys
import os
from tkcalendar import Calendar
from typing import Dict, List, Tuple

class CronVisualizerGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Cron Schedule Visualizer")
        self.root.geometry("1200x800")
        
        self.schedules = {}
        self.canvas_width = 1000
        self.canvas_height = 200
        self.hour_width = self.canvas_width // 24
        self.selected_date = datetime.now()
        self.default_duration = 20  # Duration in minutes
        
        # Add default path
        default_path = "../update_groups.json"
        if os.path.exists(default_path):
            try:
                with open(default_path, 'r') as f:
                    data = json.load(f)
                    self.schedules = data.get('schedules', {})
            except Exception as e:
                print(f"Failed to load default file: {str(e)}")

        self.setup_gui()

        if self.schedules:
            self.update_visualization()
            self.update_schedule_list()
        
    def setup_gui(self):
        # Top frame for file selection and controls
        top_frame = ttk.Frame(self.root, padding="10")
        top_frame.pack(fill=tk.X)
        
        ttk.Button(top_frame, text="Load JSON File", command=self.load_json).pack(side=tk.LEFT, padx=5)
        
        self.ignore_days_var = tk.BooleanVar()
        ttk.Checkbutton(top_frame, text="Ignore day parameters", 
                       variable=self.ignore_days_var, 
                       command=self.update_visualization).pack(side=tk.LEFT, padx=5)
        
        # Duration control
        ttk.Label(top_frame, text="Duration (minutes):").pack(side=tk.LEFT, padx=5)
        self.duration_var = tk.StringVar(value=str(self.default_duration))
        duration_entry = ttk.Entry(top_frame, textvariable=self.duration_var, width=5)
        duration_entry.pack(side=tk.LEFT, padx=5)
        duration_entry.bind('<Return>', lambda e: self.update_visualization())
        ttk.Button(top_frame, text="Update", command=self.update_visualization).pack(side=tk.LEFT, padx=5)
        
        # Calendar widget
        calendar_frame = ttk.Frame(self.root, padding="10")
        calendar_frame.pack(fill=tk.X)
        
        self.calendar = Calendar(calendar_frame, selectmode='day', 
                               year=self.selected_date.year,
                               month=self.selected_date.month,
                               day=self.selected_date.day)
        self.calendar.pack(side=tk.LEFT)
        self.calendar.bind('<<CalendarSelected>>', self.on_date_select)
        
        # Timeline canvas
        canvas_frame = ttk.Frame(self.root, padding="10")
        canvas_frame.pack(fill=tk.BOTH, expand=True)
        
        self.canvas = tk.Canvas(canvas_frame, 
                              width=self.canvas_width,
                              height=self.canvas_height,
                              bg='white')
        self.canvas.pack(fill=tk.BOTH, expand=True)
        
        # Bind mouse motion for hover effect
        self.canvas.bind('<Motion>', self.on_hover)
        
        # Schedule list
        list_frame = ttk.Frame(self.root, padding="10")
        list_frame.pack(fill=tk.BOTH, expand=True)
        
        self.schedule_list = ttk.Treeview(list_frame, columns=('Schedule', 'Times'), 
                                        show='headings')
        self.schedule_list.heading('Schedule', text='Schedule Name')
        self.schedule_list.heading('Times', text='Execution Times')
        self.schedule_list.pack(fill=tk.BOTH, expand=True)
        
        # Status bar
        self.status_var = tk.StringVar()
        status_bar = ttk.Label(self.root, textvariable=self.status_var)
        status_bar.pack(fill=tk.X, pady=5)

    def convert_7field_to_5field(self, cron_str: str) -> str:
        """Convert 7-field cron (with seconds and years) to 5-field format."""
        fields = cron_str.split()
        if len(fields) == 7:
            return ' '.join(fields[1:-1])
        return cron_str
        
    def load_json(self):
        file_path = filedialog.askopenfilename(
            filetypes=[("JSON files", "*.json"), ("All files", "*.*")])
        if not file_path:
            return
            
        try:
            with open(file_path, 'r') as f:
                data = json.load(f)
                self.schedules = data.get('schedules', {})
                self.update_visualization()
                self.update_schedule_list()
        except Exception as e:
            messagebox.showerror("Error", f"Failed to load file: {str(e)}")
    
    def get_color(self, value: int, max_value: int) -> str:
        """Generate color based on value intensity."""
        if max_value == 0:
            return "#FFFFFF"
        
        # Convert from HSV to RGB (using red hue, varying saturation)
        hue = 0  # Red
        saturation = min(value / max_value, 1.0)
        value = 1.0  # Brightness
        rgb = colorsys.hsv_to_rgb(hue, saturation, value)
        
        return f"#{int(rgb[0]*255):02x}{int(rgb[1]*255):02x}{int(rgb[2]*255):02x}"
    
    def parse_cron_schedule(self, schedule: str, target_date: datetime) -> List[datetime]:
        """Parse cron schedule and return list of times it occurs in 24 hours."""
        if self.ignore_days_var.get():
            parts = schedule.split()
            parts[3:] = ['*'] * len(parts[3:])
            schedule = ' '.join(parts)
        
        schedule = self.convert_7field_to_5field(schedule)
        base = target_date.replace(hour=0, minute=0, second=0, microsecond=0)
        next_day = base + timedelta(days=1)
        
        try:
            cron = croniter(schedule, base)
            times = []
            next_time = cron.get_next(datetime)
            
            while next_time < next_day:
                times.append(next_time)
                next_time = cron.get_next(datetime)
                
            return times
        except ValueError:
            return []
    
    def get_task_overlaps(self) -> List[List[str]]:
        """Calculate overlapping tasks for each minute of the day."""
        try:
            duration = int(self.duration_var.get())
        except ValueError:
            duration = self.default_duration
        
        # Initialize timeline with empty lists for each minute
        timeline = [[] for _ in range(24 * 60)]
        
        # For each schedule, add its task duration to the timeline
        for name, schedule in self.schedules.items():
            start_times = self.parse_cron_schedule(schedule, self.selected_date)
            
            for start_time in start_times:
                start_minute = start_time.hour * 60 + start_time.minute
                
                # Add the task name to each minute it runs
                for minute in range(start_minute, min(start_minute + duration, 24 * 60)):
                    timeline[minute].append(name)
        
        return timeline
    
    def update_visualization(self):
        self.canvas.delete('all')
        
        # Draw hour lines and labels
        for hour in range(25):
            x = hour * self.hour_width
            self.canvas.create_line(x, 0, x, self.canvas_height, fill='gray')
            if hour < 24:
                self.canvas.create_text(x + self.hour_width/2, self.canvas_height - 20,
                                     text=f"{hour:02d}:00")
        
        # Get timeline with overlaps
        timeline = self.get_task_overlaps()
        max_overlaps = max(len(tasks) for tasks in timeline)
        
        # Draw visualization
        for minute in range(24 * 60):
            hour = minute // 60
            minute_in_hour = minute % 60
            
            x = hour * self.hour_width + (minute_in_hour * self.hour_width / 60)
            count = len(timeline[minute])
            
            if count > 0:
                color = self.get_color(count, max_overlaps)
                x2 = x + self.hour_width / 60
                
                self.canvas.create_rectangle(
                    x, 20,
                    x2, self.canvas_height - 40,
                    fill=color, outline='',
                    tags=('time_slot', f'minute_{minute}', 
                        f'count_{count}', 
                        f'tasks_{"/".join(timeline[minute])}')  # Change separator to '/'
                )
        
        self.status_var.set(f"Maximum concurrent tasks: {max_overlaps}")
    
    def update_schedule_list(self):
        self.schedule_list.delete(*self.schedule_list.get_children())
        for name, schedule in self.schedules.items():
            times = self.parse_cron_schedule(schedule, self.selected_date)
            if times or self.ignore_days_var.get():
                time_str = ', '.join(t.strftime('%H:%M') for t in times)
                self.schedule_list.insert('', 'end', values=(name, time_str))
    
    def on_date_select(self, event=None):
        date = self.calendar.get_date()
        self.selected_date = datetime.strptime(date, '%m/%d/%y')
        self.update_visualization()
        self.update_schedule_list()
    
    def on_hover(self, event):
        x, y = event.x, event.y
        
        if 20 <= y <= self.canvas_height - 40:
            hour = int(x // self.hour_width)
            minute_in_hour = int((x % self.hour_width) / (self.hour_width / 60))
            minute_index = hour * 60 + minute_in_hour
            
            if 0 <= minute_index < 24 * 60:
                time_str = f"{hour:02d}:{minute_in_hour:02d}"
                items = self.canvas.find_overlapping(x-1, 20, x+1, self.canvas_height-40)
                if items:
                    for item in items:
                        tags = self.canvas.gettags(item)
                        # Fix 1: Check if we have a tasks tag before accessing index 3
                        tasks_tag = next((tag for tag in tags if tag.startswith('tasks_')), None)
                        if tasks_tag:
                            tasks = tasks_tag[6:].split('/')  # Fix 2: Change separator to '/'
                            count = len(tasks)
                            task_list = ', '.join(tasks)
                            self.status_var.set(
                                f"Time: {time_str} - {count} concurrent tasks: {task_list}")
                            break
                else:
                    self.status_var.set(f"Time: {time_str} - No tasks")

if __name__ == "__main__":
    root = tk.Tk()
    app = CronVisualizerGUI(root)
    root.mainloop()
