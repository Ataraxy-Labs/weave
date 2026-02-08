import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  basePath: "/weave",
  eslint: {
    ignoreDuringBuilds: true,
  },
};

export default nextConfig;
