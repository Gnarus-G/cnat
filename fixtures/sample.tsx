import React from "react";

export default function Foo() {
  return (
    <div
      className="sr-only visible untouched sm:[&_button]:text-sm"
      intent="karma"
    >
      <Button className="[&>.MuiButton-startIcon]:absolute">
        <span className="MuiButton-startIcon">×</span>
        Button
      </Button>

      <span className="[&>*]:w-10 [&]:last-of-type:pb-6 untouched"> </span>
    </div>
  );
}
