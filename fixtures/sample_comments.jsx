import React from "react";

/**
 * This should be preserved
 *
 */
export default function Foo() {
  return (
    // asdfasdfj
    <div
      className="sr-only visible untouched sm:[&_button]:text-sm"
      intent="karma" // This also should be preserved
    >
      {/* This also should be preserved */}
      <Button className="[&>.MuiButton-startIcon]:absolute">
        <span className="MuiButton-startIcon w-10/11">×</span>
        Button
      </Button>

      <span className="[&>*]:w-[10px] [&]:last-of-type:pb-6 untouched"> </span>
    </div>
  );
}

// asdfasdf asdf;lkj
const props = {
  className: "uppercase visible",
};
