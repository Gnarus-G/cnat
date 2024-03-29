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

export const button = React.createElement("button", {
  className: "uppercase",
});
