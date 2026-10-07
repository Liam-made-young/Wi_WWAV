// Every part of the fake core, registered by importing it. Each part of
// Heat adds one line here for its module.
import './core';
import './today';
import './tasks';
import './grades';

export { createFake, type Fake } from './core';
