import { loader } from "./loader";
const { utilities } = await loader();
export const { mean, median, std } = utilities;
