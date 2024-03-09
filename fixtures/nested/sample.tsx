import React from "react";

export default function Foo() {
  return (
    <div
      className="sr-only visible untouched sm:[&_button]:text-sm"
      intent="karma"
    >
      <Button className="[&>.MuiButton-startIcon]:absolute">
        <span className="MuiButton-startIcon w-10/11">×</span>
        Button
      </Button>

      <span className="[&>*]:w-[10px] [&]:last-of-type:pb-6 untouched"> </span>
    </div>
  );
}

export function Bar() {
  return (
    <Paper
      classes={{
        root: "bg-white",
        paper: "bg-blue-500",
      }}
      bodyClassName="bg-blue-500 text-sm"
      buttonClassName="py-2 text-sm"
    >
      {React.createElement(Dialog, {
        className: "w-10",
        classes: {
          root: "bg-blue-500 px-4",
        },
      })}
    </Paper>
  );
}
