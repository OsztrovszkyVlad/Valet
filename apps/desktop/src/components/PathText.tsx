import React from 'react';
import { splitPath } from './paths';

interface PathTextProps {
    path: string;
    /** Extra class on the `<code>` element, e.g. `destination`. */
    className?: string;
}

/**
 * A path on one line, shortened in the middle when it does not fit: the
 * start gets an ellipsis, the last folder and name stay visible. The full
 * path is in the tooltip and in the element's text.
 */
const PathText: React.FC<PathTextProps> = ({ path, className }) => {
    const { head, tail } = splitPath(path);
    return (
        <code className={`path-code path-middle${className ? ` ${className}` : ''}`} title={path}>
            <span className="path-head">{head}</span>
            <span className="path-tail">{tail}</span>
        </code>
    );
};

export default PathText;
