// Every part of the fake core, registered by importing it. Each part of
// Heat adds one line here for its module.
import './core';
import './today';
import './tasks';
import './grades';
import './types';
import './syllabus';
import './habits';
import './public';
import './claude';
import './settings';

export { createFake, type Fake } from './core';
