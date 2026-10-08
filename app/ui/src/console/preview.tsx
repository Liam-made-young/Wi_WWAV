import { createRoot } from 'react-dom/client';
import '../styles/tokens.css';
import '../focus/prism.css';
import { ConsoleView } from './ConsoleView';

// Development-only entry for Console, using the real wi-devbridge.
createRoot(document.getElementById('root')!).render(<ConsoleView active />);
