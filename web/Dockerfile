FROM node:22-bookworm

WORKDIR /app

# Copy package files first for better layer caching
COPY web/package.json web/package-lock.json* ./

RUN npm install

# Copy the rest of the application
COPY web/ .

# Build the application for production
RUN npm run build

EXPOSE 5173

# Start development server with host binding
CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0", "--port", "5173"]
