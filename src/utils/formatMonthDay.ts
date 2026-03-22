const pad2 = (value: number) => value.toString().padStart(2, "0");

export const formatMonthDay = (timestampMs: number): string => {
    const date = new Date(timestampMs);
    if (Number.isNaN(date.getTime())) return "--";
    const month = pad2(date.getMonth() + 1);
    const day = pad2(date.getDate());
    return `${month}-${day}`;
};
